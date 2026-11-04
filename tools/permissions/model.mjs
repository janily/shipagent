// Isolated approval test model; real backend owns all file writes and permission decisions.
import {createScenarioModel} from '../fixtures/model.mjs';
import {Readable} from 'node:stream';
import {pathToFileURL} from 'node:url';
export function approvalModel(root,delayMs=0,chunkMs=10){
 const server=createScenarioModel({name:'permission-allow',chunk_ms:chunkMs},root);
 const handler=server.listeners('request')[0];server.removeAllListeners('request');
 let first=true;
 server.on('request',async(req,res)=>{
  if(req.method!=='POST'){handler(req,res);return;}
  try{
   const chunks=[];let size=0;for await(const chunk of req){size+=chunk.length;if(size>8*1024*1024)throw Error('oversized fixture request');chunks.push(chunk);}
   const body=JSON.parse(Buffer.concat(chunks));
   // Reissue the write in each user round, so a remembered grant is exercised on a second call.
   const index=body.messages?.findLastIndex(m=>m.role==='user')??-1;
   if(index>=0)body.messages=body.messages.slice(index);
   if(first&&body.tools?.length){first=false;await new Promise(r=>setTimeout(r,delayMs));}
   const input=Readable.from([JSON.stringify(body)]);input.url=req.url;input.method=req.method;handler(input,res);
  }catch{res.writeHead(400);res.end();}
 });return server;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const server=approvalModel(process.env.AGENTKIT_FIXTURE_ROOT,Number(process.env.AGENTKIT_APPROVAL_DELAY_MS??0),Number(process.env.AGENTKIT_APPROVAL_CHUNK_MS??10));
 server.listen(0,'127.0.0.1',()=>console.log(`FAKE_LLM_URL=http://127.0.0.1:${server.address().port}/v1`));
 for(const signal of ['SIGTERM','SIGINT'])process.on(signal,()=>{server.closeAllConnections();server.close(()=>process.exit(0));});
}
