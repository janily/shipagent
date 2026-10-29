// A real platform window, not the test-support platform. No model or backend.
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const scratch = await mkdtemp(join(tmpdir(), 'shipagent-native-'));
const report = join(scratch, 'frame.json');
const binary = resolve(process.argv[2] ?? `target/debug/examples/components${process.platform === 'win32' ? '.exe' : ''}`);
let evidence;
try {
  await promisify(execFile)(binary, ['--smoke-report', report], { timeout: 30_000, maxBuffer: 64 * 1024 });
  evidence = JSON.parse(await readFile(report, 'utf8'));
  const platform = { darwin: 'macos', win32: 'windows', linux: 'linux' }[process.platform];
  const arch = { arm64: 'aarch64', x64: 'x86_64' }[process.arch];
  if (evidence.platform !== platform || evidence.arch !== arch || evidence.renderedFrame !== true ||
      !Number.isFinite(evidence.width) || !Number.isFinite(evidence.height) ||
      evidence.width < 960 || evidence.height < 600) throw new Error('Invalid native frame report');
  evidence = { ...evidence, status: 'passed', physicalDesktopAcceptance: 'not_evaluated' };
} catch (error) {
  evidence = { status: 'failed', platform: process.platform, error: error.message };
  process.exitCode = 1;
} finally {
  await rm(scratch, { recursive: true, force: true });
}
await writeFile('target/native-smoke.json', JSON.stringify(evidence, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify(evidence));
