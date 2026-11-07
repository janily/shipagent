import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
const linux={skip:process.platform!=='linux'};
function supervise(child){const run=spawnSync('python3',['tools/perf/supervise.py','python3','-c',child],{encoding:'utf8',timeout:15000,env:{...process.env,AGENTKIT_PERF_FOCUS_X11:'0',AGENTKIT_PERF_CRASH_AFTER_MS:'0'}});assert.ifError(run.error);return {code:run.status,report:JSON.parse(run.stdout)};}
test('already-dead adopted descendant is reaped without claiming forced termination',linux,()=>{const {code,report}=supervise('import os,time\nif os.fork()==0: os._exit(0)\ntime.sleep(.2)');assert.equal(code,0);assert.equal(report.forcedCleanup,false);assert.equal(report.remainingOwnedChildren,0);assert.equal(report.reapedOrphans,1);});
test('live adopted descendant still fails the clean-exit gate and is contained',linux,()=>{const {code,report}=supervise('import os,time\nif os.fork()==0: time.sleep(60)\nos._exit(0)');assert.equal(code,2);assert.equal(report.forcedCleanup,true);assert.equal(report.remainingOwnedChildren,0);assert.equal(report.reapedOrphans,1);});
