import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
const here = dirname(fileURLToPath(import.meta.url));
test('effective schemas and constants reproduce from pinned source with complete provenance', () => {
  const dir = mkdtempSync(join(tmpdir(), 'agentkit-schema-'));
  try {
    const run = () => {
      const result = spawnSync(process.execPath, [join(here, 'schema.mjs'), '--out', dir], { encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      return Object.fromEntries(readdirSync(dir).sort().map(name => [name, readFileSync(join(dir, name), 'utf8')]));
    };
    const first = run();
    assert.deepEqual(run(), first);
    const provenance = JSON.parse(first['manifest.json']);
    assert.equal(provenance.roots.length, 14);
    for (const [name, hash] of Object.entries(provenance.artifacts)) assert.equal(createHash('sha256').update(first[name]).digest('hex'), hash, name);
    const message = JSON.parse(first['Message.json']).definitions.Message;
    assert.ok(message.required.includes('timestamp'));
    assert.equal(message.properties.timestamp.type, 'integer');
    assert.equal(message.properties.elapsedSeconds.type, 'number');
    assert.notEqual(message.additionalProperties, false);
    const session = JSON.parse(first['Session.json']).definitions.Session;
    assert.ok(session.required.includes('workspaceName'));
    assert.equal(session.properties.messages.items.$ref, '#/definitions/Message');
    const events = JSON.parse(first['SessionEvent.json']);
    const names = events.definitions.SessionEvent.anyOf.map(e => e.$ref);
    assert.ok(names.includes('#/definitions/SessionRetryBackoffEvent'));
    assert.ok(names.includes('#/definitions/SessionRetryStateEvent'));
    assert.ok(names.includes('#/definitions/SessionToolResultEvent'));
    assert.match(first['constants.rs'], /sessions:sendMessage/);
    assert.match(first['constants.rs'], /DEFAULT_MAX_MESSAGE_BYTES: u64 = 16777216/);
    assert.ok(!Object.values(first).join('').includes(here));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
