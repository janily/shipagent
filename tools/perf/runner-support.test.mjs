import test from 'node:test';import assert from 'node:assert/strict';
import {isolatedEnvironment,cleanupVerdict,fingerprint} from './runner-support.mjs';
test('profile isolation removes inherited credentials and stale probe controls',()=>{
 const env=isolatedEnvironment({PATH:'ok',HTTP_PROXY:'keep',OPENAI_API_KEY:'secret',GITHUB_TOKEN:'token',AGENTKIT_REMOTE_URL:'remote',AGENTKIT_PERF_CRASH_AFTER_MS:'99',AGENTKIT_RECORD_EVENTS:'private'}, {AGENTKIT_CONFIG_DIR:'/fixture/data'});
 assert.deepEqual(env,{PATH:'ok',HTTP_PROXY:'keep',AGENTKIT_CONFIG_DIR:'/fixture/data'});
});
test('cleanup requires kernel proof on Linux and checked native shutdown on macOS',()=>{
 assert.equal(cleanupVerdict('linux',{forced:false,supervision:{forcedCleanup:false,remainingOwnedChildren:0,nativeExitCode:0,error:null}}).verified,true);
 assert.equal(cleanupVerdict('linux',{forced:false,supervision:{forcedCleanup:false,remainingOwnedChildren:1,nativeExitCode:0,error:null}}).verified,false);
 const mac=cleanupVerdict('darwin',{forced:false,nativeExitCode:0,shutdown:{backendShutdownPassed:true}});assert.equal(mac.verified,true);assert.equal(mac.remainingOwnedProcesses,null);
 assert.equal(cleanupVerdict('darwin',{forced:false,nativeExitCode:0}).verified,false);
 assert.equal(cleanupVerdict('darwin',{forced:true,nativeExitCode:0,shutdown:{backendShutdownPassed:true}}).verified,false);
});
test('machine or workload changes make baseline fingerprints incompatible',()=>{
 const e={platform:'linux',arch:'x64',cpu:'fixture',machineId:'shared',kernel:'1'};
 assert.equal(fingerprint(e,{tokens:600}),fingerprint({...e,irrelevantRunId:2},{tokens:600}));
 assert.notEqual(fingerprint(e,{tokens:600}),fingerprint({...e,cpu:'other'},{tokens:600}));
 assert.notEqual(fingerprint(e,{tokens:600}),fingerprint(e,{tokens:300}));
});
