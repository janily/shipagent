// Full native application + owned backend + loopback model. No seeded connections/events.
import {spawn} from 'node:child_process';import {open,mkdir,mkdtemp,realpath,readFile,writeFile,rm,readdir,stat} from 'node:fs/promises';import {tmpdir,homedir} from 'node:os';import {join,resolve} from 'node:path';import {setTimeout as delay} from 'node:timers/promises';
import {createModel} from './model.mjs';import {evaluate} from './report.mjs';import {isolatedEnvironment} from '../../tools/perf/runner-support.mjs';
const [binary,backend,output]=process.argv.slice(2);if(!binary||!backend||!output)throw Error('Usage: node tests/e2e/run.mjs WORKBENCH BACKEND NEW.json');
const destination=resolve(output);await mkdir(resolve(destination,'..'),{recursive:true});const file=await open(destination,'wx',0o600);const root=await realpath(await mkdtemp(join(tmpdir(),'shipagent-e2e-')));
const report={schemaVersion:1,status:'running',platform:process.platform,cases:[],p0Acceptance:'incomplete',physicalAcceptance:'not_evaluated',performanceAcceptance:'not_evaluated'};
const abort=new AbortController();const cancel=()=>abort.abort();process.once('SIGINT',cancel);process.once('SIGTERM',cancel);
let model;
const homeSessions=join(homedir(),'sessions');
const homeState=()=>stat(homeSessions).then(s=>({ino:s.ino,mtimeMs:s.mtimeMs}),()=>null);
const originalHome=await homeState();
async function run(profile,name){
 const env=isolatedEnvironment(process.env,{});for(const key of Object.keys(env))if(key.startsWith('AGENTKIT_E2E_'))delete env[key];
 Object.assign(env,{AGENTKIT_E2E_ROOT:profile,AGENTKIT_E2E_CASE:name,AGENTKIT_E2E_MODEL_URL:`http://127.0.0.1:${model.server.address().port}/v1`,AGENTKIT_CONFIG_DIR:join(profile,'data'),AGENTKIT_APP_DATA_DIR:join(profile,'client'),AGENTKIT_WORKING_DIR:join(profile,'fixture'),AGENTKIT_BACKEND_DIR:resolve(backend),AGENTKIT_PERF_FOCUS_X11:process.platform==='linux'?'1':'0',AGENTKIT_LANGUAGE:'zh-CN',TMPDIR:join(profile,'tmp'),TEMP:join(profile,'tmp'),TMP:join(profile,'tmp')});
 const child=process.platform==='linux'?spawn('python3',['tools/perf/supervise.py',resolve(binary)],{env,stdio:['ignore','pipe','pipe']}):spawn(resolve(binary),[],{env,stdio:['ignore','pipe','pipe']});let wire='',forced=false,timer;
 child.stdout.on('data',c=>{if(wire.length<65536)wire+=c;});child.stderr.resume();const exited=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',(code,signal)=>resolve({code,signal}));});exited.catch(()=>{});
 const onAbort=()=>child.kill('SIGTERM');abort.signal.addEventListener('abort',onAbort,{once:true});
 let native;
 try{
  const result=await Promise.race([exited,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('native-deadline')),75000);})]);if(result.code!==0||abort.signal.aborted){native={processExit:result.code,processSignal:result.signal,cleanup:process.platform==='linux'?JSON.parse(wire||'null'):null};throw Error('native-exit');}
  native=JSON.parse(await readFile(join(profile,`${name}.json`),'utf8'));const shutdown=JSON.parse(await readFile(join(profile,`${name}-shutdown.json`),'utf8'));native.shutdownPassed=shutdown.shutdownPassed===true;native.verdict=evaluate(native);
  if(process.platform==='linux'){native.cleanup=JSON.parse(wire);if(native.cleanup.forcedCleanup||native.cleanup.remainingOwnedChildren!==0||native.cleanup.nativeExitCode!==0||native.cleanup.error!==null)throw Error('cleanup-failed');}
  else native.cleanup={method:'owned-sidecar-shutdown',verified:native.shutdownPassed,remainingOwnedProcesses:null};
  const config=JSON.parse(await readFile(join(profile,'data/config.json'),'utf8'));
  if(name==='full'){native.checks['u1.browser_initialized']&&=config.browserToolEnabled===false;native.checks['u1.home_unchanged']=JSON.stringify(await homeState())===JSON.stringify(originalHome);native.checks['u1.probe_cleanup']=!(await readdir(join(profile,'tmp'))).some(f=>f.startsWith('shipagent-connection-test-'));native.checks['u1.credentials_private']=!JSON.stringify(config).includes('fixture-e2e-only')&&!(await readFile(join(profile,'client/prefs.json'),'utf8')).includes('fixture-e2e-only');if(!model.stats.readVerified||model.stats.denied<1||model.stats.rejected<1||model.stats.errors)throw Error('model-evidence');for(const f of ['allow.txt','always.txt','always-two.txt'])if(await readFile(join(profile,'fixture',f),'utf8')!=='E2E_WRITE')throw Error('write-evidence');if(await readFile(join(profile,'fixture/deny.txt')).then(()=>true,()=>false))throw Error('denied-write-executed');}
  if(name==='defer')native.checks['defer.acknowledged']&&=config.setupDeferred===true;
  if(name==='restore'||name==='defer_restore')native.checks['restore.browser_unchanged']=config.browserToolEnabled===true;
  native.verdict=evaluate(native);report.cases.push(native);if(!native.verdict.passed)throw Error('native-assertion');
 }catch{if(!report.cases.some(c=>c.case===name))report.cases.push({...native,case:name,verdict:{passed:false,reasons:[...(native?.schemaVersion===1?evaluate(native).reasons:[]),'native-process-or-evidence-failed']}});throw Error(`E2E ${name} failed`);}
 finally{
  clearTimeout(timer);abort.signal.removeEventListener('abort',onAbort);
  if(child.exitCode===null&&child.signalCode===null){forced=true;child.kill('SIGTERM');await Promise.race([exited.catch(()=>{}),delay(8000)]);if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await Promise.race([exited.catch(()=>{}),delay(2000)]);}}
  if(forced)report.status='failed';
 }
}
try{
 for(const kind of ['full','defer']){
  const profile=join(root,kind);await mkdir(join(profile,'fixture'),{recursive:true});await mkdir(join(profile,'client'));await mkdir(join(profile,'tmp'));await writeFile(join(profile,'.shipagent-e2e'),'shipagent-e2e-v1');await writeFile(join(profile,'fixture/proof.txt'),'E2E_READ_PROOF');await writeFile(join(profile,'client/prefs.json'),JSON.stringify({version:1,appearance:'light',language:'zh-CN',geometry:{x:0,y:0,width:1280,height:820,maximized:false}}));
  model=createModel(join(profile,'fixture'));await new Promise(r=>model.server.listen(0,'127.0.0.1',r));
  await run(profile,kind);const path=join(profile,'data/config.json'),config=JSON.parse(await readFile(path,'utf8'));config.browserToolEnabled=true;await writeFile(path,JSON.stringify(config));await run(profile,kind==='full'?'restore':'defer_restore');
  report.modelStats??=[];report.modelStats.push({...model.stats});model.server.closeAllConnections();await new Promise(r=>model.server.close(r));model=null;
 }
 report.status='passed';
}catch{report.status='failed';process.exitCode=1;}
finally{if(model){report.lastModelStats={...model.stats};model.server.closeAllConnections();await new Promise(r=>model.server.close(r));}process.removeListener('SIGINT',cancel);process.removeListener('SIGTERM',cancel);await file.writeFile(JSON.stringify(report,null,2)+'\n');await file.close();await rm(root,{recursive:true,force:true});console.log(JSON.stringify({output:destination,status:report.status,cases:report.cases.map(c=>({case:c.case,phase:c.phase,failure:c.failure,verdict:c.verdict})),p0Acceptance:'incomplete'}));}
