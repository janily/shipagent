import { test } from 'node:test';
import assert from 'node:assert/strict';
import { tokenize, pacedTokens, markdownFixture } from './workload.mjs';

test('fixed tokenizer preserves Markdown and emits the requested token count', () => {
  const text = markdownFixture(120);
  const tokens = tokenize(text);
  assert.equal(tokens.length,120);
  assert.equal(tokens.join(''),text);
  assert.match(text,/```rust/);
  assert.match(text,/\|/);
  assert.match(text,/\*\*/);
});

test('deadline pacing measures actual token times and never calls early tokens instantaneous', async () => {
  let now=0; const waits=[]; const sent=[];
  const result=await pacedTokens(['a ','b ','c '], { rate:50, now:()=>now, wait:async ms=>{waits.push(ms);now+=ms;}, send:async token=>{sent.push(token);} });
  assert.deepEqual(sent,['a ','b ','c ']);
  assert.deepEqual(waits,[20,20]);
  assert.equal(result.tokens,3);
  assert.equal(result.elapsedMs,40);
  assert.equal(result.observedRate,50);
});

test('a failed writer stops the model without claiming full completion', async () => {
  let count=0;
  await assert.rejects(pacedTokens(['a','b','c'],{rate:50,wait:async()=>{},send:async()=>{if (++count===2) throw new Error('closed');}}),/closed/);
  assert.equal(count,2);
});

test('invalid rate and unbounded fixture size are rejected', async () => {
  for (const rate of [0,-1,Infinity,NaN]) await assert.rejects(pacedTokens(['a'],{rate,send:async()=>{}}));
  for (const count of [0,-1,1000000,1.5]) assert.throws(()=>markdownFixture(count));
});
