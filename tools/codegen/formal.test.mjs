import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyOverrides, nameEvents, extractConstants, verifySource, assertInterfaceOverride } from './formal.mjs';

test('overrides are guarded, scoped, and reject stale or duplicate paths', () => {
  const source = { Message: { properties: { timestamp: { type: 'number' }, elapsedSeconds: { type: 'number' } } } };
  const entry = { pointer: '/Message/properties/timestamp', from: { type: 'number' }, to: { type: 'integer', format: 'int64' }, reason: 'epoch milliseconds' };
  const result = applyOverrides(structuredClone(source), [entry]);
  assert.deepEqual(result.Message.properties.timestamp, entry.to);
  assert.deepEqual(result.Message.properties.elapsedSeconds, { type: 'number' });
  assert.throws(() => applyOverrides(structuredClone(source), [entry, entry]), /duplicate/i);
  assert.throws(() => applyOverrides(structuredClone(source), [{ ...entry, pointer: '/Message/properties/missing' }]), /missing/i);
  assert.throws(() => applyOverrides(structuredClone(source), [{ ...entry, from: { type: 'string' } }]), /changed/i);
});

test('event API names survive union order changes; ambiguous new shapes fail closed', () => {
  const event = (type, phase) => ({ type: 'object', properties: { type: { const: type }, ...(phase ? { phase } : {}) } });
  const variants = [event('text_delta'), event('retry', { const: 'backoff' }), event('retry', { enum: ['active', 'end'], type: 'string' })];
  const run = (values) => { const defs = { SessionEvent: { anyOf: structuredClone(values) } }; nameEvents(defs); return defs; };
  const first = run(variants), reversed = run(variants.toReversed());
  assert.deepEqual(Object.keys(first).sort(), Object.keys(reversed).sort());
  assert.ok(first.SessionTextDeltaEvent);
  assert.ok(first.SessionRetryBackoffEvent);
  assert.ok(first.SessionRetryStateEvent);
  assert.throws(() => run([...variants, event('retry', { const: 'unknown' })]), /retry/i);
  assert.throws(() => run([...variants, event('text_delta')]), /duplicate/i);
});

test('constants are parsed statically; unsupported expressions cannot execute', () => {
  const result = extractConstants("export const RPC_CHANNELS = { sessions: { EVENT: 'session:event' } } as const", "export const PROTOCOL_VERSION = '1.0'", 'export const LIMIT = 16 * 1024; export const DEFAULT = LIMIT;');
  assert.equal(result.channels.sessions.EVENT, 'session:event');
  assert.equal(result.limits.DEFAULT, 16384);
  assert.throws(() => extractConstants("export const RPC_CHANNELS = (() => { throw new Error('executed') })()", "export const PROTOCOL_VERSION = '1.0'", ''), /literal/i);
  assert.throws(() => extractConstants("export const RPC_CHANNELS = { a: { EVENT: 'x', EVENT: 'y' } }", "export const PROTOCOL_VERSION = '1.0'", ''), /duplicate/i);
  assert.throws(() => extractConstants("export const RPC_CHANNELS = {}", "export const PROTOCOL_VERSION = '1.0'", 'export const LIMIT = Math.random()'), /unsupported/i);
});

test('source guard rejects wrong revision, gitlink and dirty input', () => {
  assert.doesNotThrow(() => verifySource('abc', 'abc', 'abc', ''));
  assert.throws(() => verifySource('abc', 'def', 'abc', ''), /revision/i);
  assert.throws(() => verifySource('abc', 'abc', 'def', ''), /gitlink/i);
  assert.throws(() => verifySource('abc', 'abc', 'abc', ' M packages/core/src/types/message.ts'), /dirty/i);
});

test('reviewed inline-import rewrite rejects changed fields and types',()=>{
  const source="export interface WorkspaceSettings { chatStarters?: import('../workspaces/types').ChatStarter[]; name?: string }";
  const override='export interface WorkspaceSettings { chatStarters?: ChatStarter[]; name?: string }';
  assert.doesNotThrow(()=>assertInterfaceOverride(source,override,'WorkspaceSettings'));
  assert.throws(()=>assertInterfaceOverride(source.replace('name?: string','name: number'),override,'WorkspaceSettings'),/source changed/);
  assert.throws(()=>assertInterfaceOverride(source.replace('../workspaces/types','../other/types'),override,'WorkspaceSettings'),/source changed/);
});
