// Real Linux native navigation against 1000 persisted sessions and two isolated workspaces.
import {createFakeLlm} from '../fake-llm/server.mjs';
import {mkdtemp,mkdir,writeFile,readFile,rm,open} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve,join} from 'node:path';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {setTimeout as delay} from 'node:timers/promises';
const exec=promisify(execFile);
const [binary,backend,seeder,path,theme='light']=process.argv.slice(2);
if(process.platform!=='linux'||!binary||!backend||!seeder||!path||!['light','dark'].includes(theme))throw Error('Usage: node tools/sidebar/smoke.mjs WORKBENCH BACKEND SEEDER NEW.json [light|dark]');
const output=await open(resolve(path),'wx',0o600);const root=await mkdtemp(join(tmpdir(),'shipagent-sidebar-'));
const report={schemaVersion:1,status:'running',theme,performanceAcceptance:'not_evaluated',physicalDesktopAcceptance:'not_evaluated'};
let model,child,window,wire='',env;
const shot=async name=>{const file=resolve(path).replace(/\.json$/,'')+`-${name}.png`;await exec('python3',['-c',"from PIL import ImageGrab;import os,sys;ImageGrab.grab(xdisplay=os.environ['DISPLAY']).save(sys.argv[1])",file],{env});return file;};
const click=async(x,y)=>{await exec('xdotool',['mousemove','--window',window,String(x),String(y)],{env});await delay(80);await exec('xdotool',['click','1'],{env});};
const key=async(...keys)=>exec('xdotool',['key','--clearmodifiers','--window',window,...keys],{env});
const wait=async(f,label)=>{for(let i=0;i<240;i++){if(await f())return;if(child&&child.exitCode!==null)throw Error('Workbench exited early');await delay(50);}throw Error(`Timed out: ${label}`);};
const prefs=async()=>JSON.parse(await readFile(join(root,'client/prefs.json'),'utf8'));
try{
 env={...process.env};for(const k of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(k)||k.startsWith('AGENTKIT_REMOTE_')||k.startsWith('AGENTKIT_PERF_')||k==='AGENTKIT_RECORD_EVENTS')delete env[k];
 model=createFakeLlm({fixturePath:join(root,'fixture/proof.txt')});await new Promise(r=>model.server.listen(0,'127.0.0.1',r));
 await exec(resolve(seeder),[root,resolve(backend),`http://127.0.0.1:${model.server.address().port}/v1`],{env,timeout:120000,maxBuffer:4096});
 const seed=JSON.parse(await readFile(join(root,'sidebar-seed.json'),'utf8'));report.seededSessions=seed.sessions;
 await mkdir(join(root,'client'));
 await writeFile(join(root,'client/prefs.json'),JSON.stringify({version:1,appearance:theme,language:'zh-CN',restore_session:true,workspace:seed.workspace,session:'not-loaded',geometry:{x:0,y:0,width:1440,height:900,maximized:false},shell:{sidebar_width:280,sidebar_open:true,right_width:360,right_open:false,panel:'files',focus_mode:false}}));
 Object.assign(env,{AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_APP_DATA_DIR:join(root,'client'),AGENTKIT_BACKEND_DIR:resolve(backend),AGENTKIT_RECORD_EVENTS:join(root,'events.jsonl')});
 child=spawn('python3',['tools/perf/supervise.py',resolve(binary)],{env,stdio:['ignore','pipe','pipe']});child.stdout.on('data',c=>{if(wire.length<65536)wire+=c;});child.stderr.resume();const exited=new Promise((r,j)=>{child.once('error',j);child.once('exit',r);});
 await wait(async()=>{window=(await exec('xdotool',['search','--onlyvisible','--name','Workbench'],{env}).catch(()=>({stdout:''}))).stdout.trim().split('\n')[0];return window;},'native window');
 await exec('xdotool',['windowfocus','--sync',window],{env});await wait(async()=> (await prefs()).session===seed.session,'bootstrap context');await delay(400);report.initialScreenshot=await shot('list');
 await click(100,214);await delay(400);await key('F2');await delay(200);await key('ctrl+a');await exec('xdotool',['type','--clearmodifiers','--window',window,'Native renamed session'],{env});await key('Return');
 await wait(async()=> (await readFile(join(root,'events.jsonl'),'utf8').catch(()=>'')).split('\n').filter(Boolean).map(JSON.parse).some(e=>e.type==='name_changed'&&e.name==='Native renamed session'),'rename acknowledgement');report.renamed=true;
 await click(100,214);await delay(400);await key('Delete');await delay(200);report.confirmationScreenshot=await shot('delete-confirm');await key('Escape');await delay(450);report.deleteCancelled=true;report.shellAfterCancel=(await prefs()).shell;
 await click(800,790);await exec('xdotool',['type','--clearmodifiers','--window',window,'Draft preserved across workspaces'],{env});
 const previousSession=(await prefs()).session;
 await delay(250);report.shellBeforeSwitch=(await prefs()).shell;await click(100,70);await delay(250);report.menuScreenshot=await shot('workspace-menu');await exec('xdotool',['mousemove','--window',window,'100','130'],{env});await delay(200);await key('Return');
 await wait(async()=> (await prefs()).workspace===seed.other,'switch workspace');report.switched=true;const firstOther=(await prefs()).session;
 await key('ctrl+n');await wait(async()=> (await prefs()).session!==firstOther,'new session after empty-workspace switch');report.createdAfterSwitch=true;await delay(200);report.otherScreenshot=await shot('other');
 await click(100,70);await delay(150);await exec('xdotool',['mousemove','--window',window,'100','104'],{env});await delay(200);await key('Return');await wait(async()=> (await prefs()).workspace===seed.workspace,'switch back');
 if((await prefs()).session!==previousSession)throw Error('Previous selected session was not restored');report.selectedSessionRestored=true;await delay(200);report.restoredScreenshot=await shot('restored');
 await key('ctrl+q');const code=await exited;if(code!==0)throw Error('Supervised application failed');report.cleanup=JSON.parse(wire);if(report.cleanup.nativeExitCode!==0||report.cleanup.forcedCleanup||report.cleanup.remainingOwnedChildren!==0)throw Error('Owned processes did not exit cleanly');report.status='passed';
}catch(e){report.status='failed';report.error=e.message;process.exitCode=1;if(window)report.failureScreenshot=await shot('failed').catch(()=>null);}
finally{if(child?.exitCode===null){child.kill('SIGTERM');await delay(400);}if(model){model.server.closeAllConnections();await new Promise(r=>model.server.close(r));}await output.writeFile(JSON.stringify(report,null,2));await output.close();await rm(root,{recursive:true,force:true});}
