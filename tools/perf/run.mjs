import { mkdtemp, mkdir, writeFile, readFile, rm, open, realpath } from 'node:fs/promises';
import { tmpdir, cpus, totalmem, release,arch } from 'node:os';
import { resolve, join, basename } from 'node:path';
import { spawn,execFile } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { createFakeLlm } from '../fake-llm/server.mjs';
import { evaluate } from './report.mjs';
import {isolatedEnvironment,cleanupVerdict,fingerprint} from './runner-support.mjs';
import {promisify} from 'node:util';
import { fileURLToPath } from 'node:url';
const exec=promisify(execFile);
const supervisor=fileURLToPath(new URL('./supervise.py',import.meta.url));

const args=process.argv.slice(2);const options={runs:3,tokens:300,scenario:'stream',focus:'system',environment:'unspecified-machine',machine:'unclassified'};
for(let index=0;index<args.length;index+=2){
 const key=args[index]?.replace(/^--/,'');const value=args[index+1];
 if(!['binary','backend','output','runs','tokens','scenario','focus','environment','machine'].includes(key)||!value)throw new Error('Usage: node tools/perf/run.mjs --binary PATH --backend DIR --output NEW.json [--runs 3] [--tokens 300] [--scenario stream|disconnect|crash] [--environment LABEL]');
 options[key]=['runs','tokens'].includes(key)?Number(value):value;
}
if(!['linux','darwin'].includes(process.platform))throw new Error('Native benchmark requires Linux or macOS');
if(process.platform!=='linux'&&(options.focus==='xdotool'||options.scenario==='crash'))throw new Error('Kernel crash supervision and xdotool require Linux');
if(!options.binary||!options.backend||!options.output||!Number.isInteger(options.runs)||options.runs<1||options.runs>10||!Number.isInteger(options.tokens)||options.tokens<64||options.tokens>1200||!['stream','disconnect','crash'].includes(options.scenario)||!['system','xdotool'].includes(options.focus))throw new Error('Invalid benchmark options');
const output=resolve(options.output);await mkdir(resolve(output,'..'),{recursive:true});
const reportFile=await open(output,'wx',0o600);
const report={schemaVersion:1,status:'running',environment:{label:options.environment,machineId:options.machine,platform:process.platform,arch:arch(),focus:options.focus,cpu:cpus()[0]?.model,cpuCount:cpus().length,memoryBytes:totalmem(),kernel:release(),displayBackend:process.platform==='darwin'?'native-macos':process.env.WAYLAND_DISPLAY?'wayland':process.env.DISPLAY?'x11':'none',vulkanDriver:(process.env.VK_DRIVER_FILES||process.env.VK_ICD_FILENAMES)?basename(process.env.VK_DRIVER_FILES||process.env.VK_ICD_FILENAMES):null},workload:{rate:50,tokens:options.tokens,tokenizer:'simulated whitespace-delimited word plus trailing whitespace',scenario:options.scenario},runs:[]};
const save=async()=>{await reportFile.truncate(0);await reportFile.write(JSON.stringify(report,null,2)+'\n',0,'utf8');};
for (const [name,path] of [['cpuQuota','/sys/fs/cgroup/cpu.max'],['memoryLimit','/sys/fs/cgroup/memory.max']]) {
 try {report.environment[name]=(await readFile(path,'utf8')).trim();}catch{report.environment[name]=null;}
}
if(process.platform==='darwin'){try{const result=await exec('/usr/sbin/system_profiler',['SPDisplaysDataType','-json'],{timeout:10000,maxBuffer:256*1024});report.environment.gpu=JSON.parse(result.stdout).SPDisplaysDataType?.map(g=>g['sppci_model']??g['spdisplays_chipset-model']??g['_name']).sort().join('|')||'unmeasured';}catch{report.environment.gpu='unmeasured';}}else{report.environment.gpu=report.environment.vulkanDriver??'unmeasured';}
report.fingerprint=fingerprint(report.environment,report.workload);
const abort=new AbortController();const cancel=()=>abort.abort();
process.once('SIGINT',cancel);process.once('SIGTERM',cancel);
try {
 await save();
 for(let run=0;run<options.runs;run++) {
  if(abort.signal.aborted)throw new Error('Benchmark cancelled');
  const root=await realpath(await mkdtemp(join(tmpdir(),'shipagent-perf-')));let model,child,exited;let timer;let forced=false;let supervisorWire='';
  try {
   await mkdir(join(root,'client'));await writeFile(join(root,'client/prefs.json'),JSON.stringify({version:1,onboarding_deferred:true,browser_initialized:true,appearance:'light',language:'zh-CN',font_size:14,stream_fade:true,geometry:{x:0,y:0,width:1280,height:820,maximized:false}}));
   await mkdir(join(root,'fixture'));await writeFile(join(root,'.shipagent-perf'),'shipagent-perf-v1',{mode:0o600});
   await writeFile(join(root,'fixture','proof.txt'),'SHIPAGENT_TOOL_FIXTURE: native perf proof\n',{mode:0o600});
   model=createFakeLlm({fixturePath:join(root,'fixture','proof.txt'),perf:{tokens:options.tokens,rate:50,disconnectAfter:options.scenario==='disconnect'?50:null}});
   await new Promise(resolve=>model.server.listen(0,'127.0.0.1',resolve));
   const env=isolatedEnvironment(process.env,{});
   Object.assign(env,{AGENTKIT_PERF_ROOT:root,AGENTKIT_PERF_CRASH_AFTER_MS:options.scenario==='crash'?'2000':'0',AGENTKIT_PERF_FOCUS_X11:options.focus==='xdotool'?'1':'0',AGENTKIT_PERF_MODEL_URL:`http://127.0.0.1:${model.server.address().port}/v1`,AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_APP_DATA_DIR:join(root,'client'),AGENTKIT_WORKING_DIR:join(root,'fixture'),AGENTKIT_BACKEND_DIR:resolve(options.backend)});
   child=process.platform==='linux'?spawn('python3',[supervisor,resolve(options.binary)],{env,stdio:['ignore','pipe','pipe']}):spawn(resolve(options.binary),[],{env,stdio:['ignore','pipe','pipe']});
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
   const failed={native,model:model?structuredClone(model.stats):null,error:'native-run-failed'};
   const evidence=evaluate(failed,options.tokens);
   failed.verdict={...evidence,valid:false,frameBudgetMet:false,reasons:[...evidence.reasons,'runner-error']};
   report.runs.push(failed);
  } finally {
   clearTimeout(timer);
   if(child && child.exitCode===null && child.signalCode===null){
     forced=true;child.kill('SIGTERM');
     if(exited)await Promise.race([exited.catch(()=>{}),delay(10000)]);
     if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');if(exited)await Promise.race([exited.catch(()=>{}),delay(2000)]);}
   }
   let supervision;
   try{supervision=JSON.parse(supervisorWire);}catch{supervision=null;}
   const last=report.runs.at(-1);
   let shutdown;try{shutdown=JSON.parse(await readFile(join(root,'shutdown.json'),'utf8'));}catch{}
   last.cleanup=cleanupVerdict(process.platform,{forced,supervision,shutdown,nativeExitCode:child?.exitCode});
   if(!last.cleanup.verified){
      last.verdict={valid:false,frameBudgetMet:false,reasons:[...(last.verdict?.reasons??[]),'forced-or-incomplete-process-cleanup'],referenceAcceptance:'not_evaluated'};
   }
   if(model){model.server.closeAllConnections();await new Promise(resolve=>model.server.close(resolve));}
   await rm(root,{recursive:true,force:true});
  }
  await save();
  // A disposable macOS machine is the final containment boundary on failure.
  // Never collect more samples on a machine with unverified backend cleanup.
  if(process.platform==='darwin'&&!report.runs.at(-1).cleanup.verified)break;
 }
 report.status=report.runs.every(run=>run.verdict.valid)?'measured':'invalid';
 await save();
 console.log(JSON.stringify({output,status:report.status,runs:report.runs.map(run=>({p95Ms:run.native?.frameWork?.p95_ms,...run.verdict,cleanup:run.cleanup}))},null,2));
 process.exitCode=report.status==='measured'&&report.runs.every(run=>run.verdict.frameBudgetMet)?0:2;
} finally {process.removeListener('SIGINT',cancel);process.removeListener('SIGTERM',cancel);await reportFile.close();}
