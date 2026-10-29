// A real platform window, not the test-support platform. No model or backend.
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { validateNativeFrame } from './report.mjs';
const scratch = await mkdtemp(join(tmpdir(), 'shipagent-native-'));
const report = join(scratch, 'frame.json');
const binary = resolve(process.argv[2] ?? `target/debug/examples/components${process.platform === 'win32' ? '.exe' : ''}`);
let evidence;
try {
  await promisify(execFile)(binary, ['--smoke-report', report], { timeout: 30_000, maxBuffer: 64 * 1024 });
  evidence = JSON.parse(await readFile(report, 'utf8'));
  evidence = validateNativeFrame(evidence);
} catch (error) {
  evidence = { status: 'failed', platform: process.platform, error: error.message, observed: evidence ?? null };
  process.exitCode = 1;
} finally {
  await rm(scratch, { recursive: true, force: true });
}
await writeFile('target/native-smoke.json', JSON.stringify(evidence, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify(evidence));
