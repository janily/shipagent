import http from 'node:http';
import { once } from 'node:events';
import { markdownFixture, tokenize, pacedTokens } from '../perf/workload.mjs';
import { pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

// Scripted model only. The real backend executes read; this server never reads the fixture.
export function createFakeLlm({ fixturePath, expectedMarker = 'SHIPAGENT_TOOL_FIXTURE', chunkMs = 12, perf = null } = {}) {
  if (!fixturePath) throw new Error('fixturePath is required');
  if (perf) { markdownFixture(perf.tokens); if (perf.rate !== 50) throw new Error('Performance workload rate must be 50'); }
  const stats = { requests: 0, toolCalls: 0, toolResults: 0, errors: 0, perfStreams: [] };
  const server = http.createServer(async (req, res) => {
    const json = (status, body) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(body)); };
    if (req.method === 'GET' && req.url === '/__stats') return json(200, stats);
    if (req.method === 'GET' && req.url === '/v1/models') return json(200, { object: 'list', data: [{ id: 'fixture-model', object: 'model', owned_by: 'local-test' }] });
    if (req.method !== 'POST' || req.url !== '/v1/chat/completions') return json(404, { error: { message: 'Unknown fake model route' } });
    try {
      const buffers = []; let size = 0;
      for await (const chunk of req) { size += chunk.length; if (size > 8 * 1024 * 1024) return json(413, { error: { message: 'Request too large' } }); buffers.push(chunk); }
      const body = JSON.parse(Buffer.concat(buffers).toString('utf8'));
      if (!Array.isArray(body.messages)) return json(400, { error: { message: 'messages must be an array' } });
      stats.requests++;
      const tool = body.messages.findLast(message => message.role === 'tool' && message.tool_call_id === 'call_fixture_read');
      if (tool && !JSON.stringify(tool.content).includes(expectedMarker)) { stats.errors++; return json(422, { error: { message: 'Expected fixture marker absent from real tool result' } }); }
      const hasTools = Array.isArray(body.tools) && body.tools.length > 0;
      const read = body.tools?.find(tool => tool.function?.name === 'read');
      if (hasTools && !read && !tool) return json(422, { error: { message: 'Backend did not provide the read tool' } });
      const text = tool && perf ? markdownFixture(perf.tokens) : tool ? `已读取验证文件。\n\n工具返回的标记是 **${expectedMarker}**。\n\n这段答复来自真实 Agent 的第二次模型调用。` : hasTools ? '我先读取工作目录中的验证文件。' : 'Fixture session';
      if (tool) stats.toolResults++;
      if (!body.stream) return json(200, { id: 'fixture-completion', object: 'chat.completion', model: body.model, choices: [{ index: 0, message: { role: 'assistant', content: text }, finish_reason: 'stop' }], usage: { prompt_tokens: 30, completion_tokens: 20, total_tokens: 50 } });
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache', connection: 'keep-alive' });
      const chunk = (delta, finish_reason = null) => res.write(`data: ${JSON.stringify({ id: 'fixture-completion', object: 'chat.completion.chunk', created: 1, model: body.model, choices: [{ index: 0, delta, finish_reason }] })}\n\n`);
      chunk({ role: 'assistant' });
      if (tool && perf) {
        let sent=0;
        const result=await pacedTokens(tokenize(text), { rate:perf.rate, send:async token=>{
          if (res.destroyed) throw new Error('Stream closed');
          if (perf.disconnectAfter && ++sent===perf.disconnectAfter) { res.destroy(); throw new Error('Controlled disconnect'); }
          if (!chunk({content:token})) await once(res,'drain',{signal:AbortSignal.timeout(5000)});
        }});
        stats.perfStreams.push(result);
      } else {
        const characters = Array.from(text);
        for (let i = 0; i < characters.length && !res.destroyed; i += 3) { chunk({ content: characters.slice(i, i + 3).join('') }); if (chunkMs) await delay(chunkMs); }
      }
      if (!tool && hasTools) {
        stats.toolCalls++;
        const args = JSON.stringify({ path: fixturePath });
        chunk({ tool_calls: [{ index: 0, id: 'call_fixture_read', type: 'function', function: { name: read.function.name, arguments: args } }] });
        chunk({}, 'tool_calls');
      } else chunk({}, 'stop');
      res.end('data: [DONE]\n\n');
    } catch { stats.errors++; if (!res.headersSent) json(400, { error: { message: 'Invalid fake model request' } }); else res.end(); }
  });
  server.requestTimeout = 30_000;
  return { server, stats };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const fixturePath = process.env.AGENTKIT_FAKE_FIXTURE;
  const { server } = createFakeLlm({ fixturePath, expectedMarker: process.env.AGENTKIT_FAKE_MARKER, chunkMs: Number(process.env.AGENTKIT_FAKE_CHUNK_MS ?? 12) });
  server.listen(Number(process.env.PORT ?? 0), '127.0.0.1', () => console.log(`FAKE_LLM_URL=http://127.0.0.1:${server.address().port}/v1`));
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => { server.closeAllConnections(); server.close(() => process.exit(0)); });
}
