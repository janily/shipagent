// Credentials are confined to a temporary macOS keychain, never package output.
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdtemp,writeFile,readFile,rm,appendFile} from 'node:fs/promises';
import {resolve,join,relative,sep,basename} from 'node:path';
import {randomBytes} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const exec=promisify(execFile);
export async function command(program,args){
  try{return await exec(program,args,{timeout:120_000,maxBuffer:1024*1024});}
  catch{throw new Error(`${basename(program)} failed; signing details were withheld`);}
}
function decode(value,kind){
  if(!value||!/^[A-Za-z0-9+/]+={0,2}$/.test(value)||value.length%4!==0)throw new Error(`Invalid ${kind} encoding`);
  const bytes=Buffer.from(value,'base64');
  if(!bytes.length||bytes.toString('base64')!==value)throw new Error(`Invalid ${kind} encoding`);
  return bytes;
}
export function credentials(env){
  const cert=decode(env.APPLE_CERTIFICATE_P12_BASE64,'certificate');
  const key=decode(env.APPLE_NOTARY_PRIVATE_KEY_BASE64,'notary key');
  if(!env.APPLE_CERTIFICATE_PASSWORD||!/^Developer ID Application: [^\r\n\0]+ \([A-Z0-9]{10}\)$/.test(env.APPLE_SIGNING_IDENTITY||'')||
     !/^[A-Z0-9]{10}$/.test(env.APPLE_NOTARY_KEY_ID||'')||!/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(env.APPLE_NOTARY_ISSUER_ID||'')||
     !key.toString('utf8').startsWith('-----BEGIN PRIVATE KEY-----\n'))throw new Error('Complete Developer ID and notary credentials are required; no engineering fallback');
  return {cert,key};
}
export async function setup(env,{run=command}={}){
  if(env.GITHUB_ACTIONS!=='true'||!env.RUNNER_TEMP||!env.GITHUB_ENV)throw new Error('Signing setup requires an isolated Actions runner');
  const {cert,key}=credentials(env);
  const root=await mkdtemp(join(resolve(env.RUNNER_TEMP),'shipagent-signing-'));
  const stateFile=join(root,'state.json'),keychain=join(root,'signing.keychain-db');
  const prior=await run('/usr/bin/security',['list-keychains','-d','user']);
  const searchList=prior.stdout.split(/\r?\n/).map(s=>s.trim()).filter(Boolean).map(s=>JSON.parse(s));
  const state={root,keychain,searchList,created:false,changed:false};
  const save=()=>writeFile(stateFile,JSON.stringify(state),{mode:0o600});
  await save();
  await appendFile(env.GITHUB_ENV,`SHIPAGENT_SIGNING_STATE_FILE=${stateFile}\n`);
  try{
    const password=randomBytes(32).toString('hex');
    await run('/usr/bin/security',['create-keychain','-p',password,keychain]);state.created=true;await save();
    await run('/usr/bin/security',['set-keychain-settings','-lut','21600',keychain]);
    await run('/usr/bin/security',['unlock-keychain','-p',password,keychain]);
    const certificate=join(root,'certificate.p12'),apiKey=join(root,'notary.p8');
    await writeFile(certificate,cert,{mode:0o600});await writeFile(apiKey,key,{mode:0o600});
    await run('/usr/bin/security',['import',certificate,'-k',keychain,'-P',env.APPLE_CERTIFICATE_PASSWORD,'-T','/usr/bin/codesign']);
    await run('/usr/bin/security',['set-key-partition-list','-S','apple-tool:,apple:,codesign:','-s','-k',password,keychain]);
    // Preserve every pre-existing keychain. Cleanup also runs after partial setup.
    state.changed=true;await save();
    await run('/usr/bin/security',['list-keychains','-d','user','-s',keychain,...searchList]);
    const identities=await run('/usr/bin/security',['find-identity','-v','-p','codesigning',keychain]);
    if(!identities.stdout.includes(`"${env.APPLE_SIGNING_IDENTITY}"`))throw new Error('Requested Developer ID identity is unavailable');
    await run('/usr/bin/xcrun',['notarytool','store-credentials','shipagent-ci','--key',apiKey,'--key-id',env.APPLE_NOTARY_KEY_ID,'--issuer',env.APPLE_NOTARY_ISSUER_ID,'--keychain',keychain]);
    await rm(certificate);await rm(apiKey);
    await appendFile(env.GITHUB_ENV,`APPLE_SIGNING_IDENTITY=${env.APPLE_SIGNING_IDENTITY}\nAPPLE_NOTARY_KEYCHAIN_PROFILE=shipagent-ci\nAPPLE_SIGNING_KEYCHAIN_PATH=${keychain}\n`);
    return stateFile;
  }catch{
    await cleanup({...env,SHIPAGENT_SIGNING_STATE_FILE:stateFile},{run});
    throw new Error('Signing credential setup failed; no engineering fallback');
  }
}
export async function cleanup(env,{run=command}={}){
  if(!env.SHIPAGENT_SIGNING_STATE_FILE)return;
  let state;
  try{state=JSON.parse(await readFile(env.SHIPAGENT_SIGNING_STATE_FILE,'utf8'));}catch(error){if(error.code==='ENOENT')return;throw new Error('Cannot read signing cleanup state');}
  const base=resolve(env.RUNNER_TEMP||''),root=resolve(state.root),rel=relative(base,root);
  if(!rel||rel==='..'||rel.startsWith(`..${sep}`)||!basename(root).startsWith('shipagent-signing-')||resolve(state.keychain)!==join(root,'signing.keychain-db'))throw new Error('Invalid signing cleanup location');
  let failed=false;
  try{if(state.changed)await run('/usr/bin/security',['list-keychains','-d','user','-s',...state.searchList]);}catch{failed=true;}
  try{if(state.created)await run('/usr/bin/security',['delete-keychain',state.keychain]);}catch{failed=true;}
  await rm(root,{recursive:true,force:true});
  if(failed)throw new Error('Signing keychain cleanup failed');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  if(process.platform!=='darwin')throw new Error('Signing keychain operations require macOS');
  if(process.argv[2]==='setup')await setup(process.env);
  else if(process.argv[2]==='cleanup')await cleanup(process.env);
  else throw new Error('Expected setup or cleanup');
}
