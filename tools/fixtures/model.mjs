import http from 'node:http';
import pathApiDefault from 'node:path';
import {readFile} from 'node:fs/promises';
import {setTimeout as delay} from 'node:timers/promises';
import {pathToFileURL} from 'node:url';
export function createScenarioModel(scenario,root,pathApi=pathApiDefault) {
 if(!root || typeof scenario.name!=='string' || !Number.isInteger(scenario.chunk_ms) || scenario.chunk_ms<0 || scenario.chunk_ms>1000) throw new Error('Invalid scripted scenario');
let calls=0;
const server=http.createServer(async(req,res)=>{
 const json=(status,data)=>{res.writeHead(status,{'content-type':'application/json'});res.end(JSON.stringify(data));};
 if(req.url==='/v1/models') return json(200,{object:'list',data:[{id:'fixture-model',object:'model'}]});
 if(req.url!=='/v1/chat/completions')return json(404,{error:{message:'Unknown fixture route'}});
 try {
 let input='';for await(const chunk of req){input+=chunk;if(input.length>8*1024*1024)return json(413,{});}
 const body=JSON.parse(input);const active=body.tools?.length>0;
 const messages=body.messages??[];
 // Unrelated historical context/tool results are not this scenario's completion.
 // Match the actual assistant call/result pair, so provider-normalized IDs work.
 const planCalls=new Set(messages.flatMap(m=>m.role==='assistant'?(m.tool_calls??[]):[]).filter(c=>c.function?.name==='mcp__session__SubmitPlan').map(c=>c.id));
 const tool=messages.some(m=>m.role==='tool'&&(scenario.name!=='plan'||planCalls.has(m.tool_call_id)));
 if(active&&scenario.name==='plan'&&process.env.AGENTKIT_FIXTURE_DIAGNOSTICS==='1') console.error('[fixture-plan] '+JSON.stringify({roles:messages.map(m=>m.role),extendedRoot:root.charCodeAt(0)===92&&root.charCodeAt(1)===92&&root[2]==='?',planPathHasForwardSlash:pathApi.join(root,'plan.md').includes('/'),toolResults:messages.filter(m=>m.role==='tool').length,planResults:messages.filter(m=>m.role==='tool'&&planCalls.has(m.tool_call_id)).length}));
 const user=JSON.stringify(messages.findLast(m=>m.role==='user')?.content??'');
 let text=active?'Fixture answer 中文。':'Fixture summary.';let tools=[];
 if(active){calls++;
 if(scenario.name==='typed-error')return json(401,{error:{message:'Fixture credentials rejected',type:'authentication_error'}});
 if(scenario.name==='retry'&&calls<=3)return json(500,{error:{message:'Fixture transient overload'}});
 if(!tool){
 const path=pathApi.join(root,'proof.txt');
 switch(scenario.name){
 case 'single-tool':tools=[['read',{path}]];break;
 case 'parallel-tools':tools=[['read',{path}],['read',{path:pathApi.join(root,'second.txt')}]];break;
 case 'tool-failure':tools=[['read',{path:pathApi.join(root,'absent.txt')}]];break;
 case 'permission-allow':case 'permission-deny':tools=[['write',{path:pathApi.join(root,'decision.txt'),content:'FIXTURE_WRITE'}]];break;
 case 'plan':tools=[['mcp__session__SubmitPlan',{planPath:pathApi.join(root,'plan.md')}]];break;
 case 'background-shell':tools=[['bash',{command:'echo FIXTURE_BACKGROUND; sleep 1; echo FIXTURE_DONE',run_in_background:true}]];break;
 case 'nested-agent':{let depth=user.includes('[nested:2]')?2:user.includes('[nested:1]')?1:0;if(depth<2)tools=[['mcp__session__spawn_session',{prompt:`[nested:${depth+1}] Complete fixture child.`,name:`Fixture child ${depth+1}`,workingDirectory:root}]];break;}
 }
 }
 }
 if(!body.stream)return json(200,{id:'fixture-model',object:'chat.completion',model:body.model,choices:[{index:0,message:{role:'assistant',content:text},finish_reason:'stop'}],usage:{prompt_tokens:20,completion_tokens:10,total_tokens:30}});
 res.writeHead(200,{'content-type':'text/event-stream','cache-control':'no-cache'});
 const chunkUsage=()=>res.write(`data: ${JSON.stringify({id:'fixture-model',choices:[],usage:{prompt_tokens:scenario.name==='compaction'&&calls===1?40000:500,completion_tokens:10,total_tokens:scenario.name==='compaction'&&calls===1?40010:510}})}\n\n`);
 const chunk=(delta,finish_reason=null)=>res.write(`data: ${JSON.stringify({id:'fixture-model',object:'chat.completion.chunk',model:body.model,choices:[{index:0,delta,finish_reason}]})}\n\n`);
 chunk({role:'assistant'});
 if(tools.length){chunk({tool_calls:tools.map(([name,args],index)=>({index,id:`fixture-call-${index}`,type:'function',function:{name,arguments:JSON.stringify(args)}}))});chunk({},'tool_calls');}
 else{for(const part of Array.from(text)){if(res.destroyed)return;chunk({content:part});if(active)await delay(scenario.chunk_ms??1);}chunk({},'stop');}
 chunkUsage();
 res.end('data: [DONE]\n\n');
 }catch{if(!res.headersSent)json(400,{error:{message:'Invalid fixture request'}});else res.end();}
});
return server;
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
 const scenario=JSON.parse(await readFile(process.argv[2],'utf8'));
 const server=createScenarioModel(scenario,process.env.AGENTKIT_FIXTURE_ROOT);
 server.listen(0,'127.0.0.1',()=>console.log(`FAKE_LLM_URL=http://127.0.0.1:${server.address().port}/v1`));
 for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>{server.closeAllConnections();server.close(()=>process.exit(0));});
}
