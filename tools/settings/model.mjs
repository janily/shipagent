import {createFakeLlm} from '../fake-llm/server.mjs';
import {join} from 'node:path';
const {server}=createFakeLlm({fixturePath:join(process.env.AGENTKIT_FIXTURE_ROOT,'proof.txt')});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
process.stdout.write(`FAKE_LLM_URL=http://127.0.0.1:${server.address().port}/v1\n`);
for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>{server.closeAllConnections();server.close(()=>process.exit(0));});
