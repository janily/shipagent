// Native first-run/deferral smoke; isolated profile, ordinary application, no perf bypass.
import {mkdtemp,mkdir,writeFile,readFile,rm,open} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {setTimeout as delay} from 'node:timers/promises';
const exec=promisify(execFile);
const [binary,backend,path,theme='light']=process.argv.slice(2);
if(process.platform!=='linux'||!binary||!backend||!path||!['light','dark'].includes(theme))throw Error('Usage: node tools/onboarding/smoke.mjs WORKBENCH BACKEND NEW.json [light|dark]');
const output=await open(resolve(path),'wx',0o600);
const root=await mkdtemp(join(tmpdir(),'shipagent-onboarding-'));
const report={schemaVersion:1,status:'running',theme,physicalDesktopAcceptance:'not_evaluated',performanceAcceptance:'not_evaluated',cleanup:[]};
const env={...process.env};
for(const k of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(k)||k.startsWith('AGENTKIT_REMOTE_')||k.startsWith('AGENTKIT_PERF_')||k==='AGENTKIT_RECORD_EVENTS')delete env[k];
Object.assign(env,{AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_APP_DATA_DIR:join(root,'client'),AGENTKIT_BACKEND_DIR:resolve(backend)});
let child,window,wire='',exited;
const prefs=async()=>JSON.parse(await readFile(join(root,'client/prefs.json'),'utf8'));
const config=async()=>JSON.parse(await readFile(join(root,'data/config.json'),'utf8'));
const wait=async(f,label)=>{for(let i=0;i<240;i++){if(await f())return;if(child&&child.exitCode!==null)throw Error('Workbench exited early');await delay(50);}throw Error(`Timed out: ${label}`);};
const shot=async name=>{const target=resolve(path).replace(/\.json$/,'')+`-${name}.png`;await exec('python3',['-c',"from PIL import ImageGrab;import os,sys;im=ImageGrab.grab(xdisplay=os.environ['DISPLAY']);assert im.getbbox(), 'Native display is blank';im.save(sys.argv[1])",target],{env});return target;};
const click=async(x,y)=>{await exec('xdotool',['mousemove','--window',window,String(x),String(y)],{env});await delay(70);await exec('xdotool',['click','1'],{env});await delay(250);};
async function launch(restoring=false){
 wire='';child=spawn('python3',['tools/perf/supervise.py',resolve(binary)],{env,stdio:['ignore','pipe','pipe']});
 child.stdout.on('data',c=>{if(wire.length<65536)wire+=c;});child.stderr.resume();
 exited=new Promise((r,j)=>{child.once('error',j);child.once('exit',r);});
 await wait(async()=>{window=(await exec('xdotool',['search','--onlyvisible','--name','Workbench'],{env}).catch(()=>({stdout:''}))).stdout.trim().split('\n')[0];return window;},'window');
 await exec('xdotool',['windowfocus','--sync',window],{env});
 await wait(async()=>prefs().then(p=>p.browser_initialized).catch(()=>false),'bootstrap');
 if(restoring)await wait(async()=>exec('python3',['-c',"from PIL import ImageGrab;import os; r,g,b=ImageGrab.grab(xdisplay=os.environ['DISPLAY']).getpixel((800,500))[:3];assert b>r+20 and b>g+20"],{env}).then(()=>true).catch(()=>false),'restored model empty state');
 await delay(350);
}
async function quit(){
 await exec('xdotool',['key','--clearmodifiers','--window',window,'ctrl+q'],{env});
 if(await exited!==0)throw Error('Application failed');
 const cleanup=JSON.parse(wire);report.cleanup.push(cleanup);
 if(cleanup.nativeExitCode!==0||cleanup.forcedCleanup||cleanup.remainingOwnedChildren!==0)throw Error('Owned processes did not exit cleanly');
}
try{
 await mkdir(join(root,'client'));
 await writeFile(join(root,'client/prefs.json'),JSON.stringify({version:1,appearance:theme,language:'zh-CN',geometry:{x:0,y:0,width:1440,height:900,maximized:false}}));
 await launch();report.welcomeScreenshot=await shot('welcome');
 if((await config()).browserToolEnabled!==false)throw Error('Browser not initialized');
 await click(500,225);report.methodsScreenshot=await shot('methods');
 await click(440,358);await delay(600);report.formScreenshot=await shot('form');
 await click(720,184);await delay(300);
 await click(1095,80);await wait(async()=> (await prefs()).onboarding_deferred===true,'defer acknowledgement');
 if((await config()).setupDeferred!==true)throw Error('Backend deferral not persisted');
 report.deferredScreenshot=await shot('deferred');await quit();
 const stored=await config();stored.browserToolEnabled=true;await writeFile(join(root,'data/config.json'),JSON.stringify(stored,null,2));
 await launch(true);
 if((await config()).browserToolEnabled!==true)throw Error('Restart overwrote browser preference');
 if((await prefs()).onboarding_deferred!==true)throw Error('Restart lost acknowledged deferral');
 report.restoredScreenshot=await shot('restored');await quit();report.status='passed';
}catch(e){report.status='failed';report.error=e.message;process.exitCode=1;if(window)report.failureScreenshot=await shot('failed').catch(()=>null);}
finally{if(child?.exitCode===null){child.kill('SIGTERM');await delay(400);}await output.writeFile(JSON.stringify(report,null,2));await output.close();await rm(root,{recursive:true,force:true});}
