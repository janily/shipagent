import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createFakeLlm } from './server.mjs';

async function withServer(run) {
  const fake = createFakeLlm({ fixturePath: '/fixture.txt', expectedMarker: 'verified-marker', chunkMs: 0 });
  await new Promise(resolve => fake.server.listen(0, '127.0.0.1', resolve));
  try { await run(`http://127.0.0.1:${fake.server.address().port}`, fake.stats); }
  finally { await new Promise(resolve => fake.server.close(resolve)); }
}
const request = (url, messages) => fetch(`${url}/v1/chat/completions`, {
  method: 'POST', headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ model: 'fixture-model', stream: true, messages, tools: [{ type: 'function', function: { name: 'read' } }] }),
});

test('streams a real read request then answers only after the expected tool result', async () => {
  await withServer(async (url, stats) => {
    const first = await request(url, [{ role: 'user', content: 'read file' }]);
    assert.equal(first.status, 200);
    const chunks = await first.text();
    assert.match(chunks, /tool_calls/); assert.match(chunks, /call_fixture_read/);
    assert.match(chunks, /\[DONE\]/); assert.match(chunks, /\/fixture.txt/);
    const final = await request(url, [{ role: 'tool', tool_call_id: 'call_fixture_read', content: 'verified-marker' }]);
    const text = await final.text();
    const answer = text.split('\n').filter(line => line.startsWith('data: {')).map(line => JSON.parse(line.slice(6)).choices[0].delta.content ?? '').join('');
    assert.match(answer, /verified-marker/); assert.equal(stats.toolResults, 1);
    assert.equal(stats.toolCalls, 1);
  });
});

test('rejects a tool result without the fixture evidence', async () => {
  await withServer(async url => {
    const result = await request(url, [{ role: 'tool', tool_call_id: 'call_fixture_read', content: 'wrong file' }]);
    assert.equal(result.status, 422);
  });
});
