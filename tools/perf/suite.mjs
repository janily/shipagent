// One reproducible suite: real Agent stream plus synthetic native UI workloads.
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdir,readFile,writeFile,mkdtemp,rm,open} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {createReadStream} from 'node:fs';
async function binaryHash(path){const hash=createHash('sha256');for await(const chunk of createReadStream(path))hash.update(chunk);return hash.digest('hex');}
import {isolatedEnvironment} from './runner-support.mjs';
import {summarize} from './compare.mjs';
const exec=promisify(execFile);const here=fileURLToPath(new URL('.',import.meta.url));
const args=process.argv.slice(2),options={runs:3,tokens:600,machine:'unclassified',focus:'system',environment:'unspecified'};
for(let i=0;i<args.length;i+=2){const key=args[i]?.slice(2);if(!['binary','backend','scenes','output','runs','tokens','machine','focus','environment'].includes(key)||!args[i+1])throw new Error('Invalid suite arguments');options[key]=['runs','tokens'].includes(key)?Number(args[i+1]):args[i+1];}
if(!options.binary||!options.backend||!options.scenes||!options.output||!Number.isInteger(options.runs)||options.runs<3||options.runs>10)throw new Error('Suite requires binaries/backend/new output and at least three runs');
const output=resolve(options.output);await mkdir(resolve(output,'..'),{recursive:true});const file=await open(output,'wx',0o600);
const root=await mkdtemp(join(tmpdir(),'shipagent-perf-suite-'));const sceneReports=[];
try{
 const streamPath=join(root,'stream.json');
 let streamExit=0;try{await exec(process.execPath,[join(here,'run.mjs'),'--binary',resolve(options.binary),'--backend',resolve(options.backend),'--output',streamPath,'--runs',String(options.runs),'--tokens',String(options.tokens),'--machine',options.machine,'--focus',options.focus,'--environment',options.environment],{timeout:options.runs*90000,maxBuffer:1024*1024});}catch(error){streamExit=error.code;if(streamExit!==2)throw new Error('Stream runner failed');}
 const stream=JSON.parse(await readFile(streamPath,'utf8'));
 const safeForScenes=stream.runs.every(r=>r.cleanup?.verified===true);
 for(let i=0;safeForScenes&&i<options.runs;i++){
  const path=join(root,`scene-${i}.json`);const env=isolatedEnvironment(process.env,{AGENTKIT_PERF_FOCUS_X11:options.focus==='xdotool'?'1':'0'});
  const command=process.platform==='linux'?'python3':resolve(options.scenes),argv=process.platform==='linux'?[join(here,'supervise.py'),resolve(options.scenes),'--report',path]:['--report',path];
  let sceneExit=0,supervision;try{const result=await exec(command,argv,{env,timeout:75000,maxBuffer:65536});if(process.platform==='linux')supervision=JSON.parse(result.stdout);}catch(error){sceneExit=error.code;}
  let report;try{report=JSON.parse(await readFile(path,'utf8'));}catch{report={status:'invalid',reason:'scene-report-unavailable'};}
  report.cleanup={method:process.platform==='linux'?'linux-kernel-subreaper':'no-backend-synthetic-scene',verified:sceneExit===0&&(process.platform!=='linux'||(supervision?.forcedCleanup===false&&supervision?.remainingOwnedChildren===0))};
  if(!report.cleanup.verified)report.status='invalid';sceneReports.push(report);
 }
 const hash=createHash('sha256');for(const path of ['run.mjs','runner-support.mjs','report.mjs','workload.mjs','suite.mjs'])hash.update(await readFile(join(here,path)));
 for(const path of ['../../crates/agentkit-app/src/perf.rs','../../crates/agentkit-ui/examples/performance.rs'])hash.update(await readFile(join(here,path)));
 const methodVersion=hash.digest('hex');
 const fingerprint=createHash('sha256').update(JSON.stringify({stream:stream.fingerprint,methodVersion,scene:sceneReports[0]?.workload,viewport:stream.runs?.[0]?.native?.viewport})).digest('hex');
 const binaries={workbenchSha256:await binaryHash(resolve(options.binary)),scenesSha256:await binaryHash(resolve(options.scenes))};
 const report={schemaVersion:2,methodVersion,fingerprint,binaries,sourceCommit:process.env.GITHUB_SHA??null,machineId:options.machine,stream,scenes:sceneReports,unmeasured:['full-workbench-process-idle-and-ten-session-rss','cold-process-start-to-input-ready','dynamic-loader-before-app-entry','physical-display-latency','seven-day-stability','reference-hardware-acceptance','macos-orphan-ownership-after-forced-exit']};
 report.summary=summarize(report);await file.writeFile(JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify({output,valid:report.summary.valid,metrics:report.summary.metrics,streamExit,referenceAcceptance:'not_evaluated'}));
 process.exitCode=report.summary.valid&&!report.summary.budgetMisses.length?streamExit:2;
}finally{await file.close();await rm(root,{recursive:true,force:true});}
