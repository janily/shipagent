import test from 'node:test';import assert from 'node:assert/strict';import {selectPrevious} from './baseline.mjs';
test('previous means latest earlier main run, including failures rather than cherry-picking success',()=>{
 const current={id:5,created_at:'2026-11-07T00:00:00Z'};
 const runs=[{id:1,head_branch:'main',event:'schedule',created_at:'2026-11-05T00:00:00Z',conclusion:'success'},{id:2,head_branch:'main',event:'schedule',created_at:'2026-11-06T00:00:00Z',conclusion:'failure'},{id:3,head_branch:'feature',event:'pull_request',created_at:'2026-11-06T01:00:00Z'},{id:6,head_branch:'main',event:'schedule',created_at:'2026-11-08T00:00:00Z'}];
 runs.push({id:4,head_branch:'main',event:'workflow_dispatch',created_at:'2026-11-06T23:00:00Z',conclusion:'success'});assert.equal(selectPrevious(runs,current).id,2);assert.equal(selectPrevious([],current),null);
});
import {downloadBaseline} from './baseline.mjs';
import {mkdtemp,readFile,rm} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';
test('latest pending run is explicit and older successful artifacts are not requested',async t=>{
 const dir=await mkdtemp(join(tmpdir(),'perf-baseline-'));t.after(()=>rm(dir,{recursive:true,force:true}));let calls=0;
 const request=async()=>({ok:true,json:async()=>++calls===1?{id:3,created_at:'2026-11-07T00:00:00Z'}:{workflow_runs:[{id:2,head_branch:'main',event:'schedule',created_at:'2026-11-06T00:00:00Z',status:'in_progress'},{id:1,head_branch:'main',event:'schedule',created_at:'2026-11-05T00:00:00Z',status:'completed'}]}});
 const result=await downloadBaseline({GITHUB_REPOSITORY:'fixture/repo',GITHUB_RUN_ID:'3',PERF_ARTIFACT_NAME:'perf-fixture',PERF_BASELINE_DIR:dir,GITHUB_TOKEN:'not-logged'},request);
 assert.equal(result.status,'previous-run-not-complete');assert.equal(calls,2);assert.equal(JSON.parse(await readFile(join(dir,'baseline-status.json'),'utf8')).runId,2);
});
test('API failure records baseline unavailable without exposing credentials',async t=>{
 const dir=await mkdtemp(join(tmpdir(),'perf-baseline-'));t.after(()=>rm(dir,{recursive:true,force:true}));
 const result=await downloadBaseline({GITHUB_REPOSITORY:'fixture/repo',GITHUB_RUN_ID:'3',PERF_ARTIFACT_NAME:'perf-fixture',PERF_BASELINE_DIR:dir,GITHUB_TOKEN:'SECRET_VALUE'},async()=>{throw new Error('SECRET_VALUE');});
 assert.equal(result.status,'baseline-unavailable');assert.doesNotMatch(await readFile(join(dir,'baseline-status.json'),'utf8'),/SECRET_VALUE/);
});
