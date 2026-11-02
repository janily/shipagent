// Lifecycle fixture, executed only by tests with the packaged Bun runtime.
const fs = require('node:fs');
const { join } = require('node:path');
const { spawn } = require('node:child_process');
const root = process.env.CONFIG_DIR;
if (process.argv.includes('--descendant')) {
  process.on('SIGTERM', () => { if (process.env.FIXTURE_MODE === 'graceful') setTimeout(() => { fs.writeFileSync(join(root, 'child-flushed'), 'yes'); process.exit(0); }, 200); });
  setInterval(() => fs.writeFileSync(join(root, 'heartbeat'), String(Date.now())), 20);
} else {
  fs.mkdirSync(root, { recursive: true });
  const attempt = Number(fs.existsSync(join(root, 'attempts')) ? fs.readFileSync(join(root, 'attempts'), 'utf8') : 0) + 1;
  fs.writeFileSync(join(root, 'attempts'), String(attempt));
  fs.writeFileSync(join(root, 'pid'), String(process.pid));
  fs.writeFileSync(join(root, 'environment.json'), JSON.stringify({ host: process.env.MKAGENT_RPC_HOST, port: process.env.MKAGENT_RPC_PORT, webui: process.env.MKAGENT_WEBUI_DIR ?? null, tls: process.env.MKAGENT_RPC_TLS_TEST ?? null, path: process.env.PATH, custom: process.env.FIXTURE_CUSTOM }));
  const token = process.env.MKAGENT_SERVER_TOKEN;
  console.log(`MKAGENT_SERVER_TOKEN=${token}`);
  console.error(`stderr token=${token}`);
  console.error(`custom key=${process.env.FIXTURE_API_KEY ?? ""}`);
  if (process.env.FIXTURE_MODE === 'occupied') { console.error('Another server instance is already running (PID 12345)'); process.exit(1); }
  if (process.env.FIXTURE_MODE === 'hang') setInterval(() => {}, 1000);
  else {
    console.error('x'.repeat(100000));
    console.log(`MKAGENT_SERVER_URL=ws://127.0.0.1:${20000 + attempt}`);
    if (process.env.FIXTURE_LOG_OCCUPIED) console.error("Another server instance is already running (PID 99999)");
    if (['descendant', 'graceful'].includes(process.env.FIXTURE_MODE)) {
      const child = spawn(process.execPath, [__filename, '--descendant'], { env: process.env, stdio: 'inherit' });
      fs.writeFileSync(join(root, 'descendant'), String(child.pid));
    }
    setInterval(() => { if (fs.existsSync(join(root, 'crash'))) { fs.unlinkSync(join(root, 'crash')); process.exit(17); } }, 10);
  }
}
