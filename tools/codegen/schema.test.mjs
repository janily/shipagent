import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const here = dirname(fileURLToPath(import.meta.url));
test('schemas reflect pinned source, retain required fields and reproduce byte for byte', () => {
  const dir = mkdtempSync(join(tmpdir(), 'agentkit-schema-'));
  try {
    const run = () => {
      const result = spawnSync(process.execPath, [join(here, 'schema.mjs'), '--out', dir], { encoding: 'utf8' });
      assert.equal(result.status, 0, result.stderr);
      return ['Message','Session','SessionEvent'].map(name => readFileSync(join(dir, `${name}.json`), 'utf8'));
    };
    const first = run();
    assert.deepEqual(run(), first);
    const message = JSON.parse(first[0]).definitions.Message;
    assert.ok(message.required.includes('timestamp'));
    assert.ok(message.properties.toolDisplayMeta);
    assert.notEqual(message.additionalProperties, false);
    const session = JSON.parse(first[1]).definitions.Session;
    assert.ok(session.required.includes('workspaceName'));
    const events = JSON.parse(first[2]).definitions.SessionEvent.anyOf;
    const retries = events.filter(e => e.properties?.type?.const === 'retry');
    assert.equal(retries.length, 2);
    assert.ok(events.some(e => e.properties?.type?.const === 'tool_result'));
    assert.ok(!first.join('').includes(here));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
