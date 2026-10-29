import { mkdtemp, mkdir, writeFile, readFile, rm, open, realpath } from 'node:fs/promises';
import { tmpdir, cpus, totalmem, release } from 'node:os';
import { resolve, join, basename } from 'node:path';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { createFakeLlm } from '../fake-llm/server.mjs';
import { evaluate } from './report.mjs';
import { fileURLToPath } from 'node:url';
const supervisor=fileURLToPath(new URL('./supervise.py',import.meta.url));

const args=process.argv.slice(2);const options={runs:3,tokens:300,scenario:'stream',focus:'system',environment:'unspecified-linux-machine'};
for(let index=0;index<args.length;index+=2){
 const key=args[index]?.replace(/^--/,'');const value=args[index+1];
 if(!['binary','backend','output','runs','tokens','scenario','focus','environment'].includes(key)||!value)throw new Error('Usage: node tools/perf/run.mjs --binary PATH --backend DIR --output NEW.json [--runs 3] [--tokens 300] [--scenario stream|disconnect|crash] [--environment LABEL]');
 options[key]=['runs','tokens'].includes(key)?Number(value):value;
}
if(process.platform!=='linux')throw new Error('This benchmark currently verifies Linux only');
if(!options.binary||!options.backend||!options.output||!Number.isInteger(options.runs)||options.runs<1||options.runs>10||!Number.isInteger(options.tokens)||options.tokens<64||options.tokens>1200||!['stream','disconnect','crash'].includes(options.scenario)||!['system','xdotool'].includes(options.focus))throw new Error('Invalid benchmark options');
const output=resolve(options.output);await mkdir(resolve(output,'..'),{recursive:true});
const reportFile=await open(output,'wx',0o600);
const report={schemaVersion:1,status:'running',environment:{label:options.environment,focus:options.focus,cpu:cpus()[0]?.model,cpuCount:cpus().length,memoryBytes:totalmem(),kernel:release(),displayBackend:process.env.WAYLAND_DISPLAY?'wayland':process.env.DISPLAY?'x11':'none',vulkanDriver:process.env.VK_DRIVER_FILES?basename(process.env.VK_DRIVER_FILES):null},workload:{rate:50,tokens:options.tokens,tokenizer:'simulated whitespace-delimited word plus trailing whitespace',scenario:options.scenario},runs:[]};
const save=async()=>{await reportFile.truncate(0);await reportFile.write(JSON.stringify(report,null,2)+'\n',0,'utf8');};
for (const [name,path] of [['cpuQuota','/sys/fs/cgroup/cpu.max'],['memoryLimit','/sys/fs/cgroup/memory.max']]) {
 try {report.environment[name]=(await readFile(path,'utf8')).trim();}catch{report.environment[name]=null;}
}
const abort=new AbortController();const cancel=()=>abort.abort();
process.once('SIGINT',cancel);process.once('SIGTERM',cancel);
try {
 await save();
 for(let run=0;run<options.runs;run++) {
  if(abort.signal.aborted)throw new Error('Benchmark cancelled');
  const root=await realpath(await mkdtemp(join(tmpdir(),'shipagent-perf-')));let model,child,exited;let timer;let forced=false;let supervisorWire='';
  try {
   await mkdir(join(root,'fixture'));await writeFile(join(root,'.shipagent-perf'),'shipagent-perf-v1',{mode:0o600});
   await writeFile(join(root,'fixture','proof.txt'),'SHIPAGENT_TOOL_FIXTURE: native perf proof\n',{mode:0o600});
   model=createFakeLlm({fixturePath:join(root,'fixture','proof.txt'),perf:{tokens:options.tokens,rate:50,disconnectAfter:options.scenario==='disconnect'?50:null}});
   await new Promise(resolve=>model.server.listen(0,'127.0.0.1',resolve));
   const env={...process.env};
   for(const key of Object.keys(env))if(/API_KEY|ACCESS_TOKEN|AUTH_TOKEN|PASSWORD|SECRET/.test(key)||key.startsWith('AGENTKIT_REMOTE_'))delete env[key];
   Object.assign(env,{AGENTKIT_PERF_ROOT:root,AGENTKIT_PERF_CRASH_AFTER_MS:options.scenario==='crash'?'2000':'0',AGENTKIT_PERF_FOCUS_X11:options.focus==='xdotool'?'1':'0',AGENTKIT_PERF_MODEL_URL:`http://127.0.0.1:${model.server.address().port}/v1`,AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_WORKING_DIR:join(root,'fixture'),AGENTKIT_BACKEND_DIR:resolve(options.backend)});
   child=spawn('python3',[supervisor,resolve(options.binary)],{env,stdio:['ignore','pipe','pipe']});
   child.stdout.on('data',chunk=>{if(supervisorWire.length<65536)supervisorWire+=chunk.toString();});
   child.stderr.resume();
   exited=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',(code,signal)=>resolve({code,signal}));});
   exited.catch(()=>{});
   const deadline=new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Native benchmark exceeded 75 seconds')),75000);});
   const cancelled=new Promise((_,reject)=>abort.signal.addEventListener('abort',()=>reject(new Error('Benchmark cancelled')),{once:true}));
   const exit=await Promise.race([exited,deadline,cancelled]);
   if(exit.code!==0)throw new Error('Native benchmark did not exit successfully');
   const native=JSON.parse(await readFile(join(root,'native.json'),'utf8'));
   const result={native,model:structuredClone(model.stats)};result.verdict=evaluate(result,options.tokens);
   report.runs.push(result);
  } catch(error) {
   let native;
   try{native=JSON.parse(await readFile(join(root,'native.json'),'utf8'));}catch{}
   const failed={native,model:model?structuredClone(model.stats):null,error:error.message};
   const evidence=evaluate(failed,options.tokens);
   failed.verdict={...evidence,valid:false,frameBudgetMet:false,reasons:[...evidence.reasons,'runner-error']};
   report.runs.push(failed);
  } finally {
   clearTimeout(timer);
   if(child && child.exitCode===null && child.signalCode===null){
     forced=true;child.kill('SIGTERM');
     if(exited)await Promise.race([exited.catch(()=>{}),delay(10000)]);
   }
   let supervision;
   try{supervision=JSON.parse(supervisorWire);}catch{supervision=null;}
   const remaining=supervision?.remainingOwnedChildren??null;
   const last=report.runs.at(-1);
   last.cleanup={forced:forced||supervision?.forcedCleanup===true,remainingOwnedProcesses:remaining,reapedOrphans:supervision?.reapedOrphans??null};
   if(forced||supervision?.forcedCleanup!==false||remaining!==0||supervision?.nativeExitCode!==0||supervision?.error!==null){
      last.verdict={valid:false,frameBudgetMet:false,reasons:[...(last.verdict?.reasons??[]),'forced-or-incomplete-process-cleanup'],referenceAcceptance:'not_evaluated'};
   }
   if(model){model.server.closeAllConnections();await new Promise(resolve=>model.server.close(resolve));}
   await rm(root,{recursive:true,force:true});
  }
  await save();
 }
 report.status=report.runs.every(run=>run.verdict.valid)?'measured':'invalid';
 await save();
 console.log(JSON.stringify({output,status:report.status,runs:report.runs.map(run=>({p95Ms:run.native?.frameWork?.p95_ms,...run.verdict,cleanup:run.cleanup}))},null,2));
 process.exitCode=report.status==='measured'&&report.runs.every(run=>run.verdict.frameBudgetMet)?0:2;
} finally {process.removeListener('SIGINT',cancel);process.removeListener('SIGTERM',cancel);await reportFile.close();}
