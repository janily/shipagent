import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,rm,stat} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {setup,cleanup,credentials,command} from './keychain.mjs';
const identity='Developer ID Application: Fixture (ABCDEFGHIJ)';
async function fixture(t){
 const root=await mkdtemp(join(tmpdir(),'shipagent-signing-test-'));t.after(()=>rm(root,{recursive:true,force:true}));
 return {RUNNER_TEMP:root,GITHUB_ENV:join(root,'env'),GITHUB_ACTIONS:'true',APPLE_CERTIFICATE_P12_BASE64:Buffer.from('fixture-certificate').toString('base64'),APPLE_CERTIFICATE_PASSWORD:'SECRET_PASSWORD',APPLE_SIGNING_IDENTITY:identity,APPLE_NOTARY_KEY_ID:'ABCDEFGHIJ',APPLE_NOTARY_ISSUER_ID:'12345678-1234-1234-1234-123456789abc',APPLE_NOTARY_PRIVATE_KEY_BASE64:Buffer.from('-----BEGIN PRIVATE KEY-----\nFIXTURE_PRIVATE_KEY\n-----END PRIVATE KEY-----\n').toString('base64')};
}
test('missing or partial credentials fail before any command or file is created',async t=>{
 const env=await fixture(t);let calls=0;
 for(const key of ['APPLE_CERTIFICATE_P12_BASE64','APPLE_CERTIFICATE_PASSWORD','APPLE_SIGNING_IDENTITY','APPLE_NOTARY_KEY_ID','APPLE_NOTARY_ISSUER_ID','APPLE_NOTARY_PRIVATE_KEY_BASE64']){
  await assert.rejects(setup({...env,[key]:''},{run:async()=>{calls++;}}));
 }
 assert.equal(calls,0);assert.throws(()=>credentials({...env,APPLE_SIGNING_IDENTITY:identity+'\nINJECT=secret'}));
});
test('temporary keychain preserves prior search list and removes raw credentials',async t=>{
 const env=await fixture(t),calls=[];
 const run=async(program,args)=>{calls.push([program,args]);if(args[0]==='list-keychains'&&!args.includes('-s'))return {stdout:'    "/Users/runner/Library/Keychains/login.keychain-db"\n'};if(args[0]==='find-identity')return {stdout:`1) FIXTURE "${identity}"`};return {stdout:''};};
 const stateFile=await setup(env,{run});const state=JSON.parse(await readFile(stateFile,'utf8'));
 await assert.rejects(stat(join(state.root,'certificate.p12')),{code:'ENOENT'});await assert.rejects(stat(join(state.root,'notary.p8')),{code:'ENOENT'});
 const output=await readFile(env.GITHUB_ENV,'utf8');assert.doesNotMatch(output,/SECRET_PASSWORD|FIXTURE_PRIVATE_KEY|fixture-certificate/);if(process.platform!=='win32')assert.equal((await stat(stateFile)).mode&0o077,0);
 await cleanup({...env,SHIPAGENT_SIGNING_STATE_FILE:stateFile},{run});
 assert.deepEqual(calls.at(-2)[1],['list-keychains','-d','user','-s','/Users/runner/Library/Keychains/login.keychain-db']);
 assert.equal(calls.at(-1)[1][0],'delete-keychain');await assert.rejects(stat(state.root),{code:'ENOENT'});
});
test('failed setup cleans imported keychain and cannot expose command secrets',async t=>{
 const env=await fixture(t),calls=[];
 const run=async(program,args)=>{calls.push(args);if(args[0]==='list-keychains'&&!args.includes('-s'))return {stdout:'"/prior.keychain-db"\n'};if(args[0]==='import')throw new Error('SECRET_PASSWORD FIXTURE_PRIVATE_KEY');return {stdout:''};};
 await assert.rejects(setup(env,{run}),error=>!error.message.includes('SECRET_PASSWORD')&&!error.message.includes('FIXTURE_PRIVATE_KEY'));
 assert.equal(calls.at(-1)[0],'delete-keychain');
 const output=await readFile(env.GITHUB_ENV,'utf8');const stateFile=output.match(/^SHIPAGENT_SIGNING_STATE_FILE=(.+)$/m)[1];await assert.rejects(stat(stateFile),{code:'ENOENT'});
});
test('process failure exposes neither arguments nor captured output',async()=>{
 await assert.rejects(command(process.execPath,['-e','process.stderr.write("DO_NOT_LEAK");process.exit(1)']),error=>!String(error).includes('DO_NOT_LEAK'));
});
