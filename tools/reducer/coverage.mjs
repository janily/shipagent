// Enforce spec03's pure store module line-coverage gate, excluding generated DTOs
// and the phase-zero compatibility projection.
import fs from 'node:fs';
import assert from 'node:assert/strict';
const report = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const moduleName = process.argv[3] ?? 'reducer';
assert.ok(['reducer', 'turns'].includes(moduleName), 'Unknown pure store module');
const pattern = new RegExp(`/agentkit-store/src/core/${moduleName}(?:\\.rs|/.*\\.rs)$`);
const files = report.data.flatMap(data => data.files).filter(file =>
  pattern.test(file.filename.replaceAll('\\', '/')));
assert.ok(files.length >= 5, `${moduleName} coverage files missing`);
let lines = 0;
for (const file of files) {
  const summary = file.summary.lines;
  assert.ok(summary.count > 0, `No instrumented lines: ${file.filename}`);
  assert.equal(summary.covered, summary.count, `Uncovered ${moduleName} lines: ${file.filename}`);
  lines += summary.count;
}
console.log(`Formal ${moduleName} line coverage: ${lines}/${lines} (100%); ${files.length} files`);
