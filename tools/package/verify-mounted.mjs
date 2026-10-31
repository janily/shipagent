// Engineering verification only: actual signed/mounted app + real bundled backend.
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, mkdir, writeFile, readFile, rename, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { createFakeLlm } from '../fake-llm/server.mjs';
import { nativeFiles } from './bundle.mjs';
import { verifyRound } from './verify-round.mjs';
const exec=promisify(execFile);
if(process.platform!=='darwin'||process.env.GITHUB_ACTIONS!=='true')throw new Error('Mounted automatic verification runs only in the macOS Actions trial workflow');
const output=resolve(process.argv[2]??'target/packages/macos-arm64');
const packaged=JSON.parse(await readFile(join(output,'package-report.json'),'utf8'));
if(!packaged.probeBuild)throw new Error('Native automatic round requires explicitly marked --probe engineering package');
const root=await realpath(await mkdtemp(join(tmpdir(),'shipagent-dmg-')));
const mount=join(root,'mount');await mkdir(mount);
let mounted=false,hidden=false,model;let failed=false;
const reportPath=join(output,'mounted-verification.json');
await writeFile(reportPath,JSON.stringify({status:'running'})+'\n',{flag:'wx'});
const source=resolve(`target/backend/darwin-${process.arch==='arm64'?'arm64':'x64'}`);
const backup=`${source}.package-verification`;
const evidence={status:'running',probeBuild:true,signMode:packaged.signMode,notarization:packaged.notarization,cleanMachineAcceptance:'not_evaluated',performanceAcceptance:'not_evaluated'};
async function hash(path){const digest=createHash('sha256');for await(const part of createReadStream(path))digest.update(part);return digest.digest('hex');}
try {
 await exec('/usr/bin/hdiutil',['attach','-readonly','-nobrowse','-mountpoint',mount,join(output,packaged.dmg.file)],{timeout:30_000});mounted=true;
 const app=join(mount,`${packaged.appName}.app`);
 const native=await nativeFiles(app);
 if(native.length!==packaged.nativeFiles.length)throw new Error('Mounted native inventory changed');
 for(const entry of packaged.nativeFiles){const file=join(app,entry.path);if(!native.includes(entry.path)||await hash(file)!==entry.sha256)throw new Error('Mounted binary checksum mismatch');await exec('/usr/bin/codesign',['--verify','--strict',file]);}
 await exec('/usr/bin/codesign',['--verify','--strict','--deep',app]);
 await rename(source,backup);hidden=true;
 await mkdir(join(root,'fixture'));
 await writeFile(join(root,'.shipagent-perf'),'shipagent-perf-v1',{mode:0o600});
 await writeFile(join(root,'fixture/proof.txt'),'SHIPAGENT_TOOL_FIXTURE: mounted signed app proof\n',{mode:0o600});
 model=createFakeLlm({fixturePath:join(root,'fixture/proof.txt')});
 await new Promise(resolve=>model.server.listen(0,'127.0.0.1',resolve));
 const env={...process.env};
 for(const key of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET/.test(key)||/^(APPLE_|OPENAI_|ANTHROPIC_|AWS_|AGENTKIT_REMOTE_|AGENTKIT_PERF_)/.test(key)||key==='AGENTKIT_BACKEND_DIR')delete env[key];
 Object.assign(env,{AGENTKIT_PERF_ROOT:root,AGENTKIT_PERF_MODEL_URL:`http://127.0.0.1:${model.server.address().port}/v1`,AGENTKIT_CONFIG_DIR:join(root,'data'),AGENTKIT_WORKING_DIR:join(root,'fixture')});
 await exec(join(app,'Contents/MacOS/workbench'),[],{cwd:join(root,'fixture'),env,timeout:65_000,maxBuffer:1024*1024});
 const report=JSON.parse(await readFile(join(root,'native.json'),'utf8'));
 verifyRound(report,model.stats);
 evidence.native=report;evidence.model=structuredClone(model.stats);evidence.sourceBackendUnavailable=true;evidence.backendOverrideUsed=false;
 // Independent persistence/cancellation/RPC contract uses the mounted signed payload.
 const cargo=process.env.CARGO??'cargo';
 await exec(cargo,['test','-p','agentkit-sidecar','--test','agent_turn','--locked','--','--ignored'],{env:{...env,AGENTKIT_BACKEND_DIR:join(app,'Contents/Resources/backend')},timeout:180_000,maxBuffer:1024*1024});
 evidence.backendContract='passed';evidence.status='passed';
} catch(error){failed=true;evidence.status='failed';evidence.error={message:error.message.split('\n')[0],code:error.code??null,signal:error.signal??null};evidence.failureProcessCleanup='not_verified';}
finally {
 if(model){model.server.closeAllConnections();await new Promise(resolve=>model.server.close(resolve));}
 if(hidden){try{await rename(backup,source);evidence.sourceBackendRestored=true;}catch{failed=true;evidence.status='failed';evidence.sourceBackendRestored=false;}}
 if(mounted){try{await exec('/usr/bin/hdiutil',['detach',mount],{timeout:30_000});evidence.dmgDetached=true;}catch{failed=true;evidence.status='failed';evidence.dmgDetached=false;}}
 if(!failed)await rm(root,{recursive:true,force:true});
 await writeFile(reportPath,JSON.stringify(evidence,null,2)+'\n');
}
console.log(JSON.stringify({status:evidence.status,dmgDetached:evidence.dmgDetached,backendContract:evidence.backendContract,sourceBackendUnavailable:evidence.sourceBackendUnavailable,error:evidence.error}));
if(failed)process.exitCode=1;
