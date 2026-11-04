// Real Linux workbench approval controls; requires an isolated X11 display.
import {approvalModel} from './model.mjs';
import {mkdtemp,mkdir,writeFile,readFile,rm,open} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {setTimeout as delay} from 'node:timers/promises';
const exec=promisify(execFile);
const [binary,backend,path,theme='light',decision='allow']=process.argv.slice(2);
if(process.platform!=='linux'||!binary||!backend||!path||!['light','dark'].includes(theme)||!['allow','deny','always'].includes(decision))throw Error('Usage: node tools/permissions/smoke.mjs BINARY BACKEND NEW.json [light|dark] [allow|deny|always]');
const output=await open(resolve(path),'wx',0o600);const root=await mkdtemp(join(tmpdir(),'shipagent-approval-'));
const report={schemaVersion:1,status:'running',theme,decision,performanceAcceptance:'not_evaluated',physicalDesktopAcceptance:'not_evaluated'};
let model,child,wire='',watchdog,env,window;
const screenshot=async suffix=>{const target=resolve(path).replace(/\.json$/,'')+suffix+'.png';await exec('python3',['-c',"from PIL import ImageGrab;import os,sys;ImageGrab.grab(xdisplay=os.environ['DISPLAY']).save(sys.argv[1])",target],{env});return target;};
try{
 await mkdir(join(root,'fixture'));await mkdir(join(root,'client'));await writeFile(join(root,'.shipagent-perf'),'shipagent-perf-v1');
 await writeFile(join(root,'client/prefs.json'),JSON.stringify({version:1,appearance:theme,language:'zh-CN',geometry:{x:0,y:0,width:1440,height:900,maximized:false},shell:{sidebar_width:280,sidebar_open:true,right_width:360,right_open:false,panel:'files',focus_mode:false}}));
 model=approvalModel(join(root,'fixture'),7000,100);await new Promise(r=>model.listen(0,'127.0.0.1',r));
 env={...process.env};for(const key of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(key)||key.startsWith('AGENTKIT_REMOTE_'))delete env[key];
 const recording=join(root,'events.jsonl');
 Object.assign(env,{AGENTKIT_PERF_ROOT:root,AGENTKIT_PERF_MODEL_URL:`http://127.0.0.1:${model.address().port}/v1`,AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_APP_DATA_DIR:join(root,'client'),AGENTKIT_WORKING_DIR:join(root,'fixture'),AGENTKIT_BACKEND_DIR:resolve(backend),AGENTKIT_PERF_FOCUS_X11:'1',AGENTKIT_RECORD_EVENTS:recording});
 child=spawn('python3',['tools/perf/supervise.py',resolve(binary)],{env,stdio:['ignore','pipe','pipe']});child.stdout.on('data',c=>{if(wire.length<65536)wire+=c;});child.stderr.resume();
 const exited=new Promise((r,j)=>{child.once('error',j);child.once('exit',r);});
 for(let i=0;i<200;i++){const found=await exec('xdotool',['search','--onlyvisible','--name','Workbench'],{env,timeout:1000}).catch(()=>({stdout:''}));window=found.stdout.trim().split('\n')[0];if(window)break;if(child.exitCode!==null)throw Error('Workbench exited before window');await delay(50);}
 if(!window)throw Error('No native window');await exec('xdotool',['windowfocus','--sync',window],{env,timeout:2000});let startup='';for(let i=0;i<300;i++){startup=await readFile(recording,'utf8').catch(()=>'');if(startup.includes('"user_message"'))break;if(child.exitCode!==null)throw Error('Workbench exited during bootstrap');await delay(50);}if(!startup.includes('"user_message"'))throw Error('No real user message after bootstrap');await delay(700);report.beforeModeScreenshot=await screenshot('-before-mode');
 await exec('xdotool',['mousemove','--window',window,'800','790','click','1'],{env});await exec('xdotool',['key','--clearmodifiers','--window',window,'shift+Tab'],{env});
 let modeEvents='';for(let i=0;i<60;i++){modeEvents=await readFile(recording,'utf8').catch(()=>'');if(modeEvents.includes('"permissionMode":"ask"'))break;await delay(50);}if(!modeEvents.includes('"permissionMode":"ask"'))throw Error('Native Shift+Tab did not confirm Ask mode');report.askModeConfirmed=true;

 let events='';for(let i=0;i<240;i++){events=await readFile(recording,'utf8').catch(()=>'');if(events.includes('"permission_request"'))break;if(child.exitCode!==null)throw Error('Agent finished before approval');await delay(50);}
 if(!events.includes('"permission_request"'))throw Error('No real approval request');await delay(500);
 report.approvalScreenshot=await screenshot('-approval');
 await exec('xdotool',['key','--clearmodifiers','--window',window,decision==='deny'?'Escape':decision==='always'?'ctrl+Return':'Return'],{env});
 if(decision!=='deny'){
  for(let i=0;i<30;i++){if(await readFile(join(root,'fixture/decision.txt'),'utf8').catch(()=>null))break;await delay(30);}
  await exec('xdotool',['mousemove','--window',window,'800','790','click','1'],{env});await exec('xdotool',['key','--clearmodifiers','--window',window,'shift+Tab'],{env});await delay(300);report.executeScreenshot=await screenshot('-execute');
 }
 const code=await Promise.race([exited,new Promise((_,reject)=>{watchdog=setTimeout(()=>reject(Error('Approval smoke timed out')),60000);})]);if(code!==0)throw Error('Supervised workbench failed');
 report.native=JSON.parse(await readFile(join(root,'native.json'),'utf8'));report.cleanup=JSON.parse(wire);report.wroteFile=await readFile(join(root,'fixture/decision.txt'),'utf8').then(v=>v==='FIXTURE_WRITE').catch(()=>false);
 const recorded=(await readFile(recording,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
 const denied=recorded.find(e=>e.type==='tool_result'&&e.isError===true&&/permission denied by user/i.test(e.result??''));
 report.denialConfirmed=decision==='deny'&&Boolean(denied);
 const terminal=report.native.events.complete===1&&!report.native.events.error&&!report.native.events.typed_error;
 // The generic performance probe marks any tool error as failed; a proven explicit denial is expected here.
 const outcome=decision==='deny'?report.denialConfirmed&&['complete','failed'].includes(report.native.outcome):report.native.outcome==='complete';
 if(!outcome||!terminal||!report.native.finalFramePresented||report.cleanup.forcedCleanup||report.cleanup.remainingOwnedChildren!==0||report.wroteFile!==(decision!=='deny'))throw Error('Approval outcome/cleanup mismatch');
 report.status='passed';
}catch(error){report.status='failed';report.error=error.message;process.exitCode=1;report.eventKinds=await readFile(join(root,'events.jsonl'),'utf8').then(t=>t.trim().split('\n').filter(Boolean).map(l=>JSON.parse(l).type)).catch(()=>[]);report.native=await readFile(join(root,'native.json'),'utf8').then(JSON.parse).catch(()=>null);if(window)report.failureScreenshot=await screenshot('-failed').catch(()=>null);}
finally{clearTimeout(watchdog);if(child?.exitCode===null)child.kill('SIGTERM');if(model){model.closeAllConnections();await new Promise(r=>model.close(r));}await output.writeFile(JSON.stringify(report,null,2));await output.close();await rm(root,{recursive:true,force:true});}
