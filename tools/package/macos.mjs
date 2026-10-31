import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, mkdir, cp, writeFile, readFile, stat, readdir, rename, rm, lstat, symlink } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { stageBundle, nativeFiles, signPlan, validateOptions, parseJitTrial } from './bundle.mjs';
const exec=promisify(execFile);
const repo=resolve(fileURLToPath(new URL('../..',import.meta.url)));
const values={};
for(let i=2;i<process.argv.length;i+=2){const key=process.argv[i];if(!['--target','--sign','--config','--probe','--notarize'].includes(key)||!process.argv[i+1])throw new Error('Invalid packaging CLI');values[key.slice(2)]=process.argv[i+1];}
if(process.platform!=='darwin')throw new Error('macOS packaging requires a native macOS host');
const config=JSON.parse(await readFile(values.config,'utf8'));
const options={...config,target:values.target,sign:values.sign,notarize:values.notarize==='true'};
validateOptions(options);
if(options.target!==`macos-${process.arch==='arm64'?'arm64':'x64'}`)throw new Error('Package target must match native host');
const identity=options.sign==='ad-hoc'?'-':process.env.APPLE_SIGNING_IDENTITY;
if(!identity || (options.sign==='developer-id'&&(!identity.startsWith('Developer ID Application:')||identity==='-')))throw new Error('Developer ID Application identity required; no ad-hoc fallback');
const profile=process.env.APPLE_NOTARY_KEYCHAIN_PROFILE;
if(options.notarize&&!profile)throw new Error('Explicit notarization requires APPLE_NOTARY_KEYCHAIN_PROFILE');
const out=join(repo,'target/packages',options.target);
await mkdir(dirname(out),{recursive:true});
try{await lstat(out);throw new Error('Package output exists; preserve or move it before rebuilding');}catch(error){if(error.code!=='ENOENT')throw error;}
const scratch=await mkdtemp(join(dirname(out),'.package-'));
async function run(program,args,extra={}) {
 try{return await exec(program,args,{timeout:120_000,maxBuffer:4*1024*1024,...extra});}
 catch(error){throw new Error(`${program} failed (${error.code??error.signal??'unknown'}): ${(error.stderr??'').slice(-1600)}`);}
}
async function hash(path){const digest=createHash('sha256');for await(const part of createReadStream(path))digest.update(part);return digest.digest('hex');}
async function bytes(path){const info=await stat(path);if(!info.isDirectory())return info.size;let total=0;for(const name of await readdir(path))total+=await bytes(join(path,name));return total;}

try {
 const bundle=join(scratch,`${config.brand.app_name}.app`);
 await stageBundle({...options,output:bundle});
 const native=await nativeFiles(bundle);const bun=join(bundle,'Contents/Resources/backend/vendor/bun/bun');
 const expectedArch=process.arch==='arm64'?'arm64':'x86_64';
 for(const path of ['Contents/MacOS/workbench','Contents/Resources/backend/vendor/bun/bun','Contents/Resources/backend/node_modules/@vscode/ripgrep/bin/rg',`Contents/Resources/backend/resources/bin/darwin-${process.arch==='arm64'?'arm64':'x64'}/uv`]) await run('/usr/bin/lipo',[join(bundle,path),'-verify_arch',expectedArch]);
 const trials=[];
 for(const [name,keys] of [['none',[]],['jit',['allow-jit']],['jit-unsigned',['allow-jit','allow-unsigned-executable-memory']]]) {
  const binary=join(scratch,`bun-${name}`);await cp(bun,binary);
  const ent=join(scratch,`trial-${name}.plist`);
  await writeFile(ent,`<?xml version="1.0"?><plist version="1.0"><dict>${keys.map(key=>`<key>com.apple.security.cs.${key}</key><true/>`).join('')}</dict></plist>`);
  await run('/usr/bin/codesign',['--force','--options','runtime','--timestamp=none','--sign','-','--entitlements',ent,binary]);
  try {const result=await exec(binary,[join(repo,'tools/package/jit-probe.mjs')],{timeout:15_000,maxBuffer:16384});trials.push({policy:name,...parseJitTrial(result.stdout)});}
  catch(error){trials.push({policy:name,passed:false,exitCode:error.code??null,signal:error.signal??null});}
 }
 if(!trials.find(x=>x.policy==='jit-unsigned')?.passed)throw new Error('Baseline Bun executable-memory trial failed');
 const signed=[];
 const emptyEntitlements=join(scratch,'no-entitlements.plist');
 await writeFile(emptyEntitlements,'<?xml version="1.0"?><plist version="1.0"><dict/></plist>');
 for(const item of signPlan(native)) {
  const path=join(bundle,item.path);
  const args=['--force','--options','runtime',options.sign==='ad-hoc'?'--timestamp=none':'--timestamp','--sign',identity];
  args.push('--entitlements',item.bun?join(repo,'packaging/macos/bun-jit.plist'):emptyEntitlements);
  await run('/usr/bin/codesign',[...args,path]);
  await run('/usr/bin/codesign',['--verify','--strict',path]);
  if(!item.app)signed.push({path:item.path,bun:item.bun});
 }
 await run('/usr/bin/codesign',['--verify','--strict','--deep',bundle]);
 if(options.notarize) {
  const archive=join(scratch,'notarization.zip');
  await run('/usr/bin/ditto',['-c','-k','--keepParent',bundle,archive]);
  const submitted=await run('/usr/bin/xcrun',['notarytool','submit',archive,'--keychain-profile',profile,'--wait','--output-format','json'],{timeout:1_800_000});
  const result=JSON.parse(submitted.stdout);if(result.status!=='Accepted')throw new Error('Apple notarization did not accept the app');
  await run('/usr/bin/xcrun',['stapler','staple',bundle]);await run('/usr/bin/xcrun',['stapler','validate',bundle]);
 }
 for(const item of signed){
  const path=join(bundle,item.path);item.sha256=await hash(path);
  const displayed=await run('/usr/bin/codesign',['-d','--entitlements',':-',path]);
  item.entitlementKeys=[...displayed.stdout.matchAll(/<key>([^<]+)<\/key>/g)].map(x=>x[1]).sort();
  const expected=item.bun?['com.apple.security.cs.allow-jit','com.apple.security.cs.allow-unsigned-executable-memory']:[];
  if(JSON.stringify(item.entitlementKeys)!==JSON.stringify(expected))throw new Error('Unexpected signed executable entitlements');
 }

 const imageSource=join(scratch,'image');await mkdir(imageSource);await run('/usr/bin/ditto',[bundle,join(imageSource,`${config.brand.app_name}.app`)]);await symlink('/Applications',join(imageSource,'Applications'));
 const result=join(scratch,'result');await mkdir(result);
 const imageName=`${config.brand.app_name}-${config.version}-${options.target}${options.probe?'-engineering':''}.dmg`;
 const dmg=join(result,imageName);
 await run('/usr/bin/hdiutil',['create','-volname',config.brand.app_name,'-srcfolder',imageSource,'-format','UDZO','-ov',dmg],{timeout:300_000});
 await run('/usr/bin/hdiutil',['verify',dmg]);
 if(options.notarize){
  await run('/usr/bin/codesign',['--sign',identity,'--timestamp',dmg]);
  const result=JSON.parse((await run('/usr/bin/xcrun',['notarytool','submit',dmg,'--keychain-profile',profile,'--wait','--output-format','json'],{timeout:1_800_000})).stdout);
  if(result.status!=='Accepted')throw new Error('Apple notarization did not accept the DMG');
  await run('/usr/bin/xcrun',['stapler','staple',dmg]);await run('/usr/bin/xcrun',['stapler','validate',dmg]);
 }

 const imageBytes=(await stat(dmg)).size;
 const report={schemaVersion:1,version:config.version,target:options.target,appName:config.brand.app_name,bundleId:config.brand.bundle_id,signMode:options.sign,hardenedRuntime:true,probeBuild:options.probe,notarization:options.notarize?'accepted_and_stapled':'not_evaluated',gatekeeperAcceptance:'not_evaluated',cleanMachineAcceptance:'not_evaluated',bunPolicy:'jit-unsigned',bunTrials:trials,nativeFiles:signed,appLogicalBytes:await bytes(bundle),dmg:{file:imageName,bytes:imageBytes,sha256:await hash(dmg),budgetBytes:220_000_000,budgetMet:imageBytes<=220_000_000}};
 await writeFile(join(result,'package-report.json'),JSON.stringify(report,null,2)+'\n');
 await writeFile(join(result,'SHA256SUMS'),`${report.dmg.sha256}  ${imageName}\n`);
 await rename(bundle,join(result,`${config.brand.app_name}.app`));
 await rename(result,out);
 console.log(JSON.stringify({output:out,dmgBytes:imageBytes,signMode:options.sign,notarization:report.notarization}));
} finally {await rm(scratch,{recursive:true,force:true});}
