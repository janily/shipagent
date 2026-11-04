// Linux native Settings smoke. Owns an isolated profile, local model and supervised application.
import {createFakeLlm} from '../fake-llm/server.mjs';
import {mkdtemp,mkdir,writeFile,readFile,rm,open} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {setTimeout as delay} from 'node:timers/promises';
const exec=promisify(execFile);const [binary,backend,seeder,path,theme='light']=process.argv.slice(2);
if(process.platform!=='linux'||!binary||!backend||!seeder||!path||!['light','dark'].includes(theme))throw Error('Usage: node tools/settings/smoke.mjs WORKBENCH BACKEND SEEDER NEW.json [light|dark]');
const output=await open(resolve(path),'wx',0o600);const root=await mkdtemp(join(tmpdir(),'shipagent-settings-'));
const report={schemaVersion:1,status:'running',theme,performanceAcceptance:'not_evaluated',physicalDesktopAcceptance:'not_evaluated'};
let child,model,window,env,wire='';
const shot=async name=>{const target=resolve(path).replace(/\.json$/,'')+`-${name}.png`;await exec('python3',['-c',"from PIL import ImageGrab;import os,sys;ImageGrab.grab(xdisplay=os.environ['DISPLAY']).save(sys.argv[1])",target],{env});return target;};
const click=async(x,y)=>{await exec('xdotool',['mousemove','--window',window,String(x),String(y)],{env});await delay(80);await exec('xdotool',['click','1'],{env});await delay(180);};
const key=async(...keys)=>exec('xdotool',['key','--clearmodifiers','--window',window,...keys],{env});
const type=async value=>exec('xdotool',['type','--clearmodifiers','--window',window,value],{env});
const prefs=async()=>JSON.parse(await readFile(join(root,'client/prefs.json'),'utf8'));
const wait=async(f,label)=>{for(let i=0;i<240;i++){if(await f())return;if(child&&child.exitCode!==null)throw Error('Workbench exited early');await delay(50);}throw Error(`Timed out: ${label}`);};
try{
 env={...process.env};for(const k of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(k)||k.startsWith('AGENTKIT_REMOTE_')||k.startsWith('AGENTKIT_PERF_')||k==='AGENTKIT_RECORD_EVENTS')delete env[k];
 model=createFakeLlm({fixturePath:join(root,'proof.txt')});await new Promise(r=>model.server.listen(0,'127.0.0.1',r));
 await exec(resolve(seeder),[root,resolve(backend),`http://127.0.0.1:${model.server.address().port}/v1`],{env,timeout:30000,maxBuffer:4096});
 const seed=JSON.parse(await readFile(join(root,'settings-seed.json'),'utf8'));await mkdir(join(root,'client'));
 await writeFile(join(root,'client/prefs.json'),JSON.stringify({version:1,appearance:theme,language:'zh-CN',restore_session:true,workspace:seed.workspace,session:'not-loaded',geometry:{x:0,y:0,width:1440,height:900,maximized:false}}));
 Object.assign(env,{AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_APP_DATA_DIR:join(root,'client'),AGENTKIT_BACKEND_DIR:resolve(backend)});
 child=spawn('python3',['tools/perf/supervise.py',resolve(binary)],{env,stdio:['ignore','pipe','pipe']});child.stdout.on('data',c=>{if(wire.length<65536)wire+=c;});child.stderr.resume();const exited=new Promise((r,j)=>{child.once('error',j);child.once('exit',r);});
 await wait(async()=>{window=(await exec('xdotool',['search','--onlyvisible','--name','Workbench'],{env}).catch(()=>({stdout:''}))).stdout.trim().split('\n')[0];return window;},'native window');await exec('xdotool',['windowfocus','--sync',window],{env});await wait(async()=> (await prefs()).session===seed.session,'bootstrap context');
 await click(800,790);await type('Draft preserved through settings');await key('ctrl+comma');await delay(600);report.modelsScreenshot=await shot('models');
 // Coordinates are fixed by the isolated 1440x900 fixture and Kit's sidebar layout.
 const coordinates={general:[70,137],appearance:[70,168],models:[70,230],english:[343,197],dark:[425,197],light:[370,197],font16:[421,319],fade:[800,418],edit:[272,362],save:[1360,884]};
 await click(...coordinates.general);report.generalScreenshot=await shot('general');await click(...coordinates.english);await wait(async()=> (await prefs()).language==='en','live English preference');report.languageChanged=true;
 await click(...coordinates.appearance);report.appearanceScreenshot=await shot('appearance');const targetTheme=theme==='light'?'dark':'light';await click(...coordinates[targetTheme]);await wait(async()=> (await prefs()).appearance===targetTheme,'live theme preference');report.themeChanged=true;
 await click(...coordinates.font16);await wait(async()=> (await prefs()).font_size===16,'font preference');await click(...coordinates.fade);await wait(async()=> (await prefs()).stream_fade===false,'fade preference');report.displaySaved=true;
 await click(...coordinates.models);await delay(250);report.localizedScreenshot=await shot('localized');
 await click(...coordinates.edit);await delay(350);report.editorScreenshot=await shot('editor');await key('ctrl+a');await type('Native settings renamed');await click(...coordinates.save);await wait(async()=>{const config=JSON.parse(await readFile(join(root,'data/config.json'),'utf8'));return config.llmConnections?.some(c=>c.name==='Native settings renamed');},'connection rename persisted');report.connectionRenamed=true;report.savedScreenshot=await shot('saved');
 await click(100,60);await delay(250);report.returnedScreenshot=await shot('returned');report.finalPreferences=await prefs();
 await key('ctrl+q');const code=await exited;if(code!==0)throw Error('Supervised application failed');report.cleanup=JSON.parse(wire);if(report.cleanup.nativeExitCode!==0||report.cleanup.forcedCleanup||report.cleanup.remainingOwnedChildren!==0)throw Error('Owned processes did not exit cleanly');report.status='passed';
}catch(e){report.status='failed';report.error=e.message;process.exitCode=1;if(window)report.failureScreenshot=await shot('failed').catch(()=>null);}
finally{if(child?.exitCode===null){child.kill('SIGTERM');await delay(400);}if(model){model.server.closeAllConnections();await new Promise(r=>model.server.close(r));}await output.writeFile(JSON.stringify(report,null,2));await output.close();await rm(root,{recursive:true,force:true});}
