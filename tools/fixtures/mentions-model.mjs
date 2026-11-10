// Observe actual provider input without changing the pinned scenario model.
import {createScenarioModel} from './model.mjs';
import {readFile, appendFile} from 'node:fs/promises';
const scenario = JSON.parse(await readFile(process.argv[2], 'utf8'));
const server = createScenarioModel(scenario, process.env.AGENTKIT_FIXTURE_ROOT);
const [handler] = server.listeners('request');
server.removeAllListeners('request');
server.on('request', async (req, res) => {
  if (req.url !== '/v1/chat/completions') return handler(req, res);
  let body = '';
  for await (const chunk of req) body += chunk;
  await appendFile(process.env.AGENTKIT_MENTION_EVIDENCE, body + '\n');
  const replay = {url: req.url, async *[Symbol.asyncIterator]() {yield body;}};
  return handler(replay, res);
});
server.listen(0, '127.0.0.1', () => console.log(`FAKE_LLM_URL=http://127.0.0.1:${server.address().port}/v1`));
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => {
  server.closeAllConnections();
  server.close(() => process.exit(0));
});
