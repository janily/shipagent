// Linux real-window confirmation proof. Uses an isolated pinned transport, never a model/provider.
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdtemp,open,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {randomUUID} from 'node:crypto';
import {setTimeout as delay} from 'node:timers/promises';
const exec=promisify(execFile);const [binary,backend,transport,path]=process.argv.slice(2);
if(process.platform!=='linux'||!path)throw Error('Usage: node tools/client-caps/smoke.mjs EXAMPLE BACKEND TRANSPORT NEW.json');
const output=await open(resolve(path),'wx',0o600);const scratch=await mkdtemp(join(tmpdir(),'shipagent-caps-'));
const report={status:'running',physicalDesktopAcceptance:'not_evaluated',performanceAcceptance:'not_evaluated'};
let server,child,window,wire='';const env={...process.env};
for(const k of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(k))delete env[k];
Object.assign(env,{SHIPAGENT_CONTRACT_TOKEN:randomUUID(),CONFIG_DIR:scratch});
const wait=async(f,label)=>{for(let i=0;i<300;i++){if(await f())return;await delay(40);}throw Error(`Timed out: ${label}`);};
const shot=async name=>{const target=resolve(path).replace(/\.json$/,'')+`-${name}.png`;await exec('python3',['-c',"from PIL import ImageGrab;import os,sys;im=ImageGrab.grab(xdisplay=os.environ['DISPLAY']);assert im.getbbox();im.save(sys.argv[1])",target],{env});return target;};
try {
 server=spawn(join(resolve(backend),'vendor/bun/bun'),[resolve(transport)],{env,stdio:['ignore','pipe','ignore']});
 let ready='';server.stdout.on('data',c=>{ready+=c;});await wait(()=>ready.includes('CONTRACT_URL='),'server');
 env.AGENTKIT_CAP_SMOKE_URL=ready.match(/CONTRACT_URL=(\S+)/)[1];
 const native=join(scratch,'native.json');
 child=spawn('python3',['tools/perf/supervise.py',resolve(binary),native],{env,stdio:['ignore','pipe','ignore']});child.stdout.on('data',c=>{wire+=c;});const exited=new Promise((r,j)=>{child.once('error',j);child.once('exit',r);});
 await wait(async()=>{window=(await exec('xdotool',['search','--onlyvisible','--name','Native capability verification'],{env}).catch(()=>({stdout:''}))).stdout.trim().split('\n')[0];return window;},'window');
 await exec('xdotool',['windowfocus','--sync',window],{env});await wait(()=>readFile(native.replace(/\.json$/,'.stage1')).then(()=>true).catch(()=>false),'first confirmation');await delay(300);report.defaultScreenshot=await shot('default');
 await exec('xdotool',['key','--clearmodifiers','--window',window,'Return'],{env});await wait(()=>readFile(native.replace(/\.json$/,'.stage2')).then(()=>true).catch(()=>false),'second confirmation');await delay(300);report.cancelScreenshot=await shot('cancel');
 await exec('xdotool',['key','--clearmodifiers','--window',window,'Escape'],{env});
 if(await exited!==0)throw Error('Native fixture failed');report.native=JSON.parse(await readFile(native,'utf8'));report.cleanup=JSON.parse(wire);
 if(report.native.status!=='passed'||report.cleanup.nativeExitCode!==0||report.cleanup.forcedCleanup||report.cleanup.remainingOwnedChildren!==0)throw Error('Response or cleanup mismatch');report.status='passed';
}catch(e){process.exitCode=1;report.status='failed';report.error=e.message;report.native=await readFile(join(scratch,'native.json'),'utf8').then(JSON.parse).catch(()=>null);}
finally{if(child?.exitCode===null){child.kill('SIGTERM');await delay(300);}if(server){server.kill('SIGTERM');await new Promise(r=>server.exitCode!==null?r():server.once('exit',r));}await output.writeFile(JSON.stringify(report,null,2));await output.close();await rm(scratch,{recursive:true,force:true});}
