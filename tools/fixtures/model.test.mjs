import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createScenarioModel} from './model.mjs';
async function run(name,fn){
 const server=createScenarioModel({name,chunk_ms:0},'/fixture');await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const request=(extra={})=>fetch(`http://127.0.0.1:${server.address().port}/v1/chat/completions`,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({model:'fixture-model',stream:true,messages:[{role:'user',content:'fixture'}],tools:[{function:{name:'read'}}],...extra})});
 try{await fn(request);}finally{server.closeAllConnections();await new Promise(r=>server.close(r));}
}
test('scripted parallel tools keep distinct identities and await real backend results',()=>run('parallel-tools',async request=>{
 const wire=await(await request()).text();assert.match(wire,/fixture-call-0/);assert.match(wire,/fixture-call-1/);assert.match(wire,/second.txt/);
 const final=await(await request({messages:[{role:'tool',content:'FIXTURE_READ'}]})).text();assert.doesNotMatch(final,/tool_calls/);assert.match(final,/\[DONE\]/);
}));
test('three transient failures reach Agent retry beyond HTTP internal retries',()=>run('retry',async request=>{
 // Nonstream title requests do not consume the scripted failure sequence.
 assert.equal((await request({tools:[],stream:false})).status,200);
 for(let i=0;i<3;i++)assert.equal((await request()).status,500);
 assert.equal((await request()).status,200);
}));
test('typed error is terminal authentication failure',()=>run('typed-error',async request=>{assert.equal((await request()).status,401);}));
test('nested prompts terminate at two child levels',()=>run('nested-agent',async request=>{
 assert.match(await(await request()).text(),/spawn_session/);
 assert.doesNotMatch(await(await request({messages:[{role:'user',content:'[nested:2]'}]})).text(),/spawn_session/);
}));
test('rejects invalid pacing and missing fixture root',()=>{
 assert.throws(()=>createScenarioModel({name:'text',chunk_ms:-1},'/fixture'));
 assert.throws(()=>createScenarioModel({name:'text',chunk_ms:1},''));
});
