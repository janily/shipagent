import test from 'node:test';
import assert from 'node:assert/strict';
import { verifyRound } from './verify-round.mjs';
const native={profile:'release',optLevel:'3',outcome:'complete',events:{text_delta:12,tool_start:1,tool_result:1,complete:1},deltaBytes:120,finalAssistantBytes:100,finalFramePresented:true};
const model={requests:3,toolCalls:1,toolResults:1,errors:0};
test('mounted package verification requires actual tool and a completed native frame',()=>{
 assert.doesNotThrow(()=>verifyRound(native,model));
 for(const candidate of [{...native,outcome:'failed'},{...native,profile:'debug'},{...native,finalFramePresented:false},{...native,events:{complete:1}}])assert.throws(()=>verifyRound(candidate,model));
 assert.throws(()=>verifyRound(native,{...model,toolResults:0}));
 assert.throws(()=>verifyRound(native,{...model,errors:1}));
});
