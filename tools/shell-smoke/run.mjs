// Linux real-workbench smoke: two launches, native shortcuts, preference persistence,
// real Read/streamed answer and kernel-owned child cleanup. No performance acceptance.
import {mkdtemp,mkdir,writeFile,readFile,rm,open} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {setTimeout as delay} from 'node:timers/promises';
import {createFakeLlm} from '../fake-llm/server.mjs';
const exec=promisify(execFile);
if(process.platform!=='linux')throw new Error('Linux shell smoke requires X11; platform IPC/native controls are tested in CI');
const [binaryArg,backendArg,reportArg,theme='light']=process.argv.slice(2);
if(!binaryArg||!backendArg||!reportArg||!['light','dark'].includes(theme))throw new Error('Usage: node tools/shell-smoke/run.mjs BINARY BACKEND NEW.json [light|dark]');
const output=await open(resolve(reportArg),'wx',0o600);
const root=await mkdtemp(join(tmpdir(),'shipagent-shell-'));
let model,child,exited;let wire='';let error;let watchdog;
const report={schemaVersion:1,theme,status:'running',performanceAcceptance:'not_evaluated',physicalDesktopAcceptance:'not_evaluated'};
try {
 await mkdir(join(root,'fixture'));await mkdir(join(root,'client'));
 await writeFile(join(root,'.shipagent-perf'),'shipagent-perf-v1');await writeFile(join(root,'fixture/proof.txt'),'SHIPAGENT_TOOL_FIXTURE: shell smoke\n');
 await writeFile(join(root,'client/prefs.json'),JSON.stringify({version:1,appearance:theme,language:'zh-CN',geometry:{x:0,y:0,width:1440,height:900,maximized:false},shell:{sidebar_width:300,sidebar_open:true,right_width:360,right_open:false,panel:'files',focus_mode:false}}));
 model=createFakeLlm({fixturePath:join(root,'fixture/proof.txt'),perf:{tokens:512,rate:50}});await new Promise(r=>model.server.listen(0,'127.0.0.1',r));
 const env={...process.env};for(const key of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(key)||key.startsWith('AGENTKIT_REMOTE_')||key==='AGENTKIT_RECORD_EVENTS')delete env[key];
 Object.assign(env,{AGENTKIT_PERF_ROOT:root,AGENTKIT_PERF_MODEL_URL:`http://127.0.0.1:${model.server.address().port}/v1`,AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_APP_DATA_DIR:join(root,'client'),AGENTKIT_WORKING_DIR:join(root,'fixture'),AGENTKIT_BACKEND_DIR:resolve(backendArg),AGENTKIT_PERF_FOCUS_X11:'1',AGENTKIT_PERF_CRASH_AFTER_MS:'0'});
 child=spawn('python3',['tools/perf/supervise.py',resolve(binaryArg)],{env,stdio:['ignore','pipe','pipe']});child.stdout.on('data',c=>{if(wire.length<65536)wire+=c;});child.stderr.resume();
 exited=new Promise((r,j)=>{child.once('error',j);child.once('exit',code=>r(code));});
 let windowId;
 for(let i=0;i<150;i++){const found=await exec('xdotool',['search','--onlyvisible','--name','Workbench'],{env,timeout:1000}).catch(()=>({stdout:''}));windowId=found.stdout.trim().split('\n')[0];if(windowId)break;if(child.exitCode!==null)throw new Error('Workbench exited before window');await delay(50);}
 if(!windowId)throw new Error('No native workbench window');
 await exec('xdotool',['windowfocus','--sync',windowId],{env,timeout:2000});
 await exec(resolve(binaryArg),[],{env,timeout:5000,maxBuffer:4096});report.secondLaunch='focus-forwarded';
 await exec('xdotool',['key','--window',windowId,'ctrl+shift+b'],{env,timeout:2000});
 let saved;
 for(let i=0;i<40;i++){saved=JSON.parse(await readFile(join(root,'client/prefs.json'),'utf8'));if(saved.shell.right_open)break;await delay(50);}
 if(!saved.shell.right_open)throw new Error('Native context-panel shortcut did not persist');report.shortcutPersisted=true;
 // Keep the native window available for a separate screenshot capture by the caller.
 if(process.env.AGENTKIT_SHELL_READY_FILE){for(let i=0;i<200&&model.stats.toolResults<1;i++)await delay(50);if(model.stats.toolResults<1)throw new Error('Read tool did not finish before screenshot');}
 if(process.env.AGENTKIT_SHELL_READY_FILE)await writeFile(process.env.AGENTKIT_SHELL_READY_FILE,JSON.stringify({windowId,theme}));
 const code=await Promise.race([exited,new Promise((_,reject)=>{watchdog=setTimeout(()=>reject(new Error('Shell smoke timeout')),60000);})]);if(code!==0)throw new Error('Supervised workbench failed');
 const native=JSON.parse(await readFile(join(root,'native.json'),'utf8'));const cleanup=JSON.parse(wire);
 saved=JSON.parse(await readFile(join(root,'client/prefs.json'),'utf8'));
 if(native.outcome!=='complete'||!native.finalFramePresented||model.stats.toolCalls!==1||model.stats.toolResults!==1)throw new Error('Native Agent round incomplete');
 if(cleanup.forcedCleanup||cleanup.remainingOwnedChildren!==0)throw new Error('Owned processes left behind');
 if(saved.appearance!==theme||!saved.shell.right_open||!saved.workspace||!saved.session)throw new Error('Final preference state mismatch');
 Object.assign(report,{status:'passed',native,cleanup,model:structuredClone(model.stats),preferences:{appearance:saved.appearance,rightOpen:saved.shell.right_open,sidebarWidth:saved.shell.sidebar_width,restoredGeometry:saved.geometry?.width===1440,lastSessionSaved:Boolean(saved.session)}});
} catch(e){error=e;report.status='failed';report.error=e.message;process.exitCode=1;}
finally {
 clearTimeout(watchdog);
 if(child?.exitCode===null){child.kill('SIGTERM');await Promise.race([exited.catch(()=>{}),delay(7000)]);}
 if(model){model.server.closeAllConnections();await new Promise(r=>model.server.close(r));}
 await output.write(JSON.stringify(report,null,2)+'\n');await output.close();await rm(root,{recursive:true,force:true});
}
console.log(JSON.stringify({status:report.status,theme,error:error?.message}));
