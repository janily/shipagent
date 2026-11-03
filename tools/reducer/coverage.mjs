// Enforce spec03's pure reducer line-coverage gate without counting generated DTOs
// or the phase-zero compatibility projection as the new reducer implementation.
import fs from 'node:fs';
import assert from 'node:assert/strict';
const report = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const files = report.data.flatMap(data => data.files).filter(file =>
  /\/agentkit-store\/src\/core\/reducer(?:\.rs|\/.*\.rs)$/.test(file.filename.replaceAll('\\', '/')));
assert.ok(files.length >= 5, 'Reducer coverage files missing');
let lines = 0;
for (const file of files) {
  const summary = file.summary.lines;
  assert.ok(summary.count > 0, `No instrumented lines: ${file.filename}`);
  assert.equal(summary.covered, summary.count, `Uncovered reducer lines: ${file.filename}`);
  lines += summary.count;
}
console.log(`Formal reducer line coverage: ${lines}/${lines} (100%); ${files.length} files`);
