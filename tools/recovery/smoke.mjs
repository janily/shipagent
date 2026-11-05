// Linux ordinary-workbench crash recovery proof; isolated restored profile, no provider credentials.
import {mkdtemp,mkdir,writeFile,readFile,rm,open} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {setTimeout as delay} from 'node:timers/promises';
const exec=promisify(execFile);const [binary,backend,path]=process.argv.slice(2);
if(process.platform!=='linux'||!path)throw Error('Usage: node tools/recovery/smoke.mjs WORKBENCH BACKEND NEW.json');
const output=await open(resolve(path),'wx',0o600);const root=await mkdtemp(join(tmpdir(),'shipagent-recovery-'));
const report={status:'running',physicalDesktopAcceptance:'not_evaluated',performanceAcceptance:'not_evaluated'};
const env={...process.env};for(const k of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(k)||k.startsWith('AGENTKIT_REMOTE_')||k.startsWith('AGENTKIT_PERF_')||k==='AGENTKIT_RECORD_EVENTS')delete env[k];
Object.assign(env,{AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_APP_DATA_DIR:join(root,'client'),AGENTKIT_BACKEND_DIR:resolve(backend)});
let child,window,wire='',exited;
const prefs=async()=>JSON.parse(await readFile(join(root,'client/prefs.json'),'utf8'));
const wait=async(f,label)=>{for(let i=0;i<300;i++){if(await f())return;if(child?.exitCode!==null&&child)throw Error('Workbench exited early');await delay(50);}throw Error(`Timed out: ${label}`);};
const shot=async name=>{const target=resolve(path).replace(/\.json$/,'')+`-${name}.png`;await exec('python3',['-c',"from PIL import ImageGrab;import os,sys;im=ImageGrab.grab(xdisplay=os.environ['DISPLAY']);assert im.getbbox();im.save(sys.argv[1])",target],{env});return target;};
const key=async k=>{await exec('xdotool',['key','--clearmodifiers','--window',window,k],{env});await delay(220);};
const click=async(x,y)=>{await exec('xdotool',['mousemove','--window',window,String(x),String(y),'click','1'],{env});await delay(250);};
try{
 await mkdir(join(root,'client'));await writeFile(join(root,'client/prefs.json'),JSON.stringify({version:1,appearance:'light',language:'zh-CN',onboarding_deferred:true,browser_initialized:true,geometry:{x:0,y:0,width:1440,height:900,maximized:false}}));
 child=spawn('python3',['tools/perf/supervise.py',resolve(binary)],{env,stdio:['ignore','pipe','pipe']});child.stdout.on('data',c=>{if(wire.length<65536)wire+=c;});child.stderr.resume();exited=new Promise((r,j)=>{child.once('error',j);child.once('exit',r);});
 await wait(async()=>{window=(await exec('xdotool',['search','--onlyvisible','--name','Workbench'],{env}).catch(()=>({stdout:''}))).stdout.trim().split('\n')[0];return window;},'window');await exec('xdotool',['windowfocus','--sync',window],{env});await wait(()=>prefs().then(p=>!!p.session).catch(()=>false),'bootstrap');await delay(600);
 const initial=(await prefs()).session;
 const lockPath=join(root,'data/.server.lock');
 const owned=JSON.parse(await readFile(lockPath,'utf8'));
 if(!Number.isSafeInteger(owned.pid)||owned.pid<=1)throw Error('Invalid owned PID');
 report.beforeScreenshot=await shot('before');
 process.kill(owned.pid,'SIGKILL');
 await delay(250);report.restartingScreenshot=await shot('restarting');
 await wait(async()=>{const lock=await readFile(lockPath,'utf8').then(JSON.parse).catch(()=>null);return lock&&lock.pid!==owned.pid;},'owned backend restart');
 await delay(1500);
 if((await prefs()).session!==initial)throw Error('Active session changed during recovery');
 report.recoveredScreenshot=await shot('recovered');
 // New conversation is gated by the controller until recovery refresh commits.
 await key('ctrl+n');await wait(async()=>(await prefs()).session!==initial,'new conversation after recovery');
 report.backendReplacementPassed=true;report.sessionPreserved=true;report.commandsRestored=true;
 await key('ctrl+q');if(await exited!==0)throw Error('Application failed');report.cleanup=JSON.parse(wire);if(report.cleanup.nativeExitCode!==0||report.cleanup.forcedCleanup||report.cleanup.remainingOwnedChildren!==0)throw Error('Owned process cleanup mismatch');report.status='passed';
}catch(e){report.status='failed';report.error=e.message;process.exitCode=1;if(window)report.failureScreenshot=await shot('failed').catch(()=>null);}
finally{if(child?.exitCode===null){child.kill('SIGTERM');await delay(400);}await output.writeFile(JSON.stringify(report,null,2));await output.close();await rm(root,{recursive:true,force:true});}
