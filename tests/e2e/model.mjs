// Only supplies model responses. Read/Write execute in the real bundled Agent.
import http from 'node:http';import {join} from 'node:path';import {setTimeout as delay} from 'node:timers/promises';
export function createModel(fixture){
 const stats={requests:0,rejected:0,readVerified:false,denied:0,toolRequests:0,toolResults:0,streamAborts:0,errors:0};
 const server=http.createServer(async(req,res)=>{
  const json=(code,value)=>{res.writeHead(code,{'content-type':'application/json'});res.end(JSON.stringify(value));};
  if(req.url.startsWith('/v1/reject')){stats.rejected++;return json(401,{error:{message:'Fixture rejected',type:'authentication_error'}});}
  if(req.method==='GET'&&req.url==='/v1/models')return json(200,{object:'list',data:[{id:'fixture-model',object:'model',owned_by:'e2e-local'}]});
  if(req.method!=='POST'||req.url!=='/v1/chat/completions')return json(404,{});
  try{
   let raw='';for await(const c of req){raw+=c;if(raw.length>8*1024*1024)return json(413,{});}const body=JSON.parse(raw);if(!Array.isArray(body.messages))return json(400,{});stats.requests++;
   const user=JSON.stringify(body.messages.findLast(m=>m.role==='user')?.content??'');const mode=Array.isArray(body.tools)?[...user.matchAll(/\[e2e:(\w+)\]/g)].at(-1)?.[1]:undefined;
   const result=id=>body.messages.findLast(m=>m.role==='tool'&&m.tool_call_id===id);
   const available=name=>body.tools?.some(t=>t.function?.name===name);
   let text='Fixture validation',call;
   if(mode==='read'){
    const tool=result('e2e-read');if(tool){if(!JSON.stringify(tool.content).includes('E2E_READ_PROOF')){stats.errors++;return json(422,{});}stats.readVerified=true;stats.toolResults++;text='## READ_OK\n\n**E2E_READ_PROOF**\n\n```rust\nlet checked = true;\n```';}
    else call=['e2e-read','read',{path:join(fixture,'proof.txt')}];
   }else if(['deny','allow','always'].includes(mode)){
    const id=`e2e-${mode}`,first=result(id),second=result('e2e-always-two');
    if(!first)call=[id,'write',{path:join(fixture,`${mode}.txt`),content:'E2E_WRITE'}];
    else if(mode==='always'&&!second)call=['e2e-always-two','write',{path:join(fixture,'always-two.txt'),content:'E2E_WRITE'}];
    else{stats.toolResults++;if(mode==='deny'&&/permission denied by user/i.test(JSON.stringify(first.content)))stats.denied++;text=`${mode.toUpperCase()}_OK`;}
   }else if(mode==='continue')text='CONTINUE_OK';
   else if(mode==='cancel'||mode==='crash')text='slow '.repeat(1000);
   if(call&&!available(call[1])){stats.errors++;return json(422,{error:{message:'Required tool absent'}});}
   if(!body.stream)return json(200,{id:'e2e',object:'chat.completion',model:body.model,choices:[{index:0,message:{role:'assistant',content:text},finish_reason:'stop'}],usage:{prompt_tokens:20,completion_tokens:10,total_tokens:30}});
   res.writeHead(200,{'content-type':'text/event-stream','cache-control':'no-cache'});
   const chunk=(delta,finish_reason=null)=>res.write(`data: ${JSON.stringify({id:'e2e',object:'chat.completion.chunk',model:body.model,choices:[{index:0,delta,finish_reason}]})}\n\n`);
   chunk({role:'assistant'});
   if(call){stats.toolRequests++;chunk({content:'Checking fixture.'});chunk({tool_calls:[{index:0,id:call[0],type:'function',function:{name:call[1],arguments:JSON.stringify(call[2])}}]});chunk({},'tool_calls');}
   else{for(let i=0;i<text.length;i+=3){if(res.destroyed){stats.streamAborts++;return;}chunk({content:text.slice(i,i+3)});await delay(mode==='cancel'||mode==='crash'?20:4);}chunk({},'stop');}
   res.end('data: [DONE]\n\n');
  }catch{if(res.destroyed){stats.streamAborts++;return;}stats.errors++;if(!res.headersSent)json(400,{});else res.end();}
 });server.requestTimeout=30000;return {server,stats};
}
