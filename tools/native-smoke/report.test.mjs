import test from 'node:test';
import assert from 'node:assert/strict';
import { validateNativeFrame } from './report.mjs';
const host = { platform: 'win32', arch: 'x64' };
const frame = { platform: 'windows', arch: 'x86_64', renderedFrame: true, submittedFrames: 1, width: 1280, height: 820 };
test('callback without a submitted frame cannot pass native smoke', () => {
  assert.throws(() => validateNativeFrame({ ...frame, submittedFrames: 0 }, host));
  const { submittedFrames, ...oldReport } = frame;
  assert.throws(() => validateNativeFrame(oldReport, host));
});
test('native report must match host architecture and usable window bounds', () => {
  assert.throws(() => validateNativeFrame(frame, { platform: 'darwin', arch: 'arm64' }));
  assert.throws(() => validateNativeFrame({ ...frame, width: 800 }, host));
  assert.throws(() => validateNativeFrame({ ...frame, height: NaN }, host));
  assert.throws(() => validateNativeFrame(null, host));
  assert.throws(() => validateNativeFrame(frame, { platform: 'unknown', arch: 'x64' }));
});
test('submitted native frame stays distinct from physical desktop acceptance', () => {
  const valid = validateNativeFrame(frame, host);
  assert.equal(valid.status, 'passed');
  assert.equal(valid.submittedFrames, 1);
  assert.equal(valid.physicalDesktopAcceptance, 'not_evaluated');
});
test('failure names the rejected evidence field and preserves safe values', () => {
  assert.throws(() => validateNativeFrame({ ...frame, submittedFrames: 0 }, host), /submittedFrames=0/);
  assert.throws(() => validateNativeFrame({ ...frame, height: 580 }, host), /height=580/);
});
