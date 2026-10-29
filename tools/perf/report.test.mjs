import {test} from 'node:test';
import assert from 'node:assert/strict';
import {evaluate} from './report.mjs';
function sample(){return {native:{profile:'release',optLevel:'3',activeUpdateFraction:1,outcome:'complete',events:{complete:1,tool_start:1,tool_result:1},finalAssistantBytes:2000,finalFramePresented:true,firstDeltaToTerminalMs:6200,frameWork:{count:200,p95_ms:10},unmeasuredFrames:0,unpresentedFrames:0,unpairedPresentFrames:0},model:{errors:0,toolCalls:1,toolResults:1,perfStreams:[{tokens:300,bytes:2000,elapsedMs:5980,observedRate:50,maxGapMs:22,completed:true}]}};}
test('a complete, correctly paced release run can meet the local frame budget',()=>{assert.deepEqual(evaluate(sample(),300),{valid:true,frameBudgetMet:true,reasons:[],referenceAcceptance:'not_evaluated'});});
test('missing samples, a debug build and incomplete visible text cannot pass',()=>{
 for(const mutate of [s=>s.native.activeUpdateFraction=0,s=>s.native.frameWork.count=0,s=>s.native.profile='debug',s=>s.native.outcome='failed',s=>s.native.finalFramePresented=false,s=>s.native.finalAssistantBytes=1999,s=>s.native.frameWork.p95_ms=null,s=>s.native.unmeasuredFrames=200]){
  const s=sample();mutate(s);const result=evaluate(s,300);assert.equal(result.valid,false);assert.equal(result.frameBudgetMet,false);
 }
});
test('zero/slow/bursty input and missing real tool evidence invalidate the measurement',()=>{
 for(const mutate of [s=>s.model.perfStreams=[],s=>s.model.perfStreams[0].observedRate=30,s=>s.model.perfStreams[0].maxGapMs=500,s=>s.model.perfStreams[0].tokens=100,s=>s.model.toolResults=0,s=>s.model.errors=1]){
  const s=sample();mutate(s);assert.equal(evaluate(s,300).valid,false);
 }
});
test('a valid measurement above 16 ms is reported as a failed budget, never discarded',()=>{const s=sample();s.native.frameWork.p95_ms=17;assert.equal(evaluate(s,300).valid,true);assert.equal(evaluate(s,300).frameBudgetMet,false);});
