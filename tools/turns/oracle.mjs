// Execute the pinned pure upstream handlers and the seven original chat assertion suites.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
const require = createRequire(import.meta.url);
const ts = require('../codegen/node_modules/typescript');
const repo = path.resolve(import.meta.dirname, '../..');
const upstream = path.join(repo, 'backend/mkagent');
const pin = '6e8ebc456b6b82441be09118ca11afca79b84836';
assert.equal(execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim(),pin,'Wrong upstream revision');
assert.ok(execFileSync('git',['ls-files','--stage','backend/mkagent'],{cwd:repo,encoding:'utf8'}).includes(pin),'Wrong staged gitlink');
assert.equal(execFileSync('git',['status','--porcelain','--untracked-files=all'],{cwd:upstream,encoding:'utf8'}).trim(),'','Dirty upstream source');
const processor = path.join(upstream, 'packages/ui/src/components/chat');
const tests = path.join(processor, '__tests__');
process.env.TZ = 'UTC';
let randomSeed = 1;
const seededMath = Object.create(Math);
seededMath.random = () => ((randomSeed = (Math.imul(randomSeed,1664525) + 1013904223) >>> 0) / 4294967296);
const fixedNow = 1700000000000;
let source, title, assertions = 0;
const records = [], cases = [], cache = new Map(), readSources = new Set();
const clone = value => value === undefined ? null : JSON.parse(JSON.stringify(value, (_key,v) => { if (Object.prototype.toString.call(v) === '[object Set]') return [...v]; if (typeof v === 'number' && (!Number.isFinite(v) || Object.is(v,-0))) return {$number:Object.is(v,-0) ? '-0' : String(v)}; return v; }));
class Clock extends Date { constructor(...args) { super(...(args.length ? args : [fixedNow])); } static now() { return fixedNow; } }
function match(actual, expected, partial = false) {
  if (Object.prototype.toString.call(expected) === '[object RegExp]') return typeof actual === 'string' && expected.test(actual);
  if (Object.prototype.toString.call(expected) === '[object Set]') return Object.prototype.toString.call(actual) === '[object Set]' && actual.size === expected.size && [...expected].every(v => actual.has(v));
  if (Array.isArray(expected)) return Array.isArray(actual) && actual.length === expected.length && expected.every((v,i)=>match(actual[i],v));
  if (expected && typeof expected === 'object') return !!actual && typeof actual === 'object' && (partial || Object.keys(actual).length === Object.keys(expected).length) && Object.keys(expected).every(k=>match(actual[k],expected[k],partial));
  return Object.is(actual, expected);
}
function expect(actual, negate = false) {
  const check = ok => { assertions++; assert.equal(negate ? !ok : ok, true, `${source}: ${title}`); };
  return { get not() { return expect(actual, !negate); },
    toBe(v) { check(Object.is(actual,v)); }, toEqual(v) { check(match(actual,v)); },
    toMatchObject(v) { check(match(actual,v,true)); }, toBeNull() { check(actual === null); },
    toBeUndefined() { check(actual === undefined); }, toBeDefined() { check(actual !== undefined); }, toHaveLength(n) { check(actual.length === n); },
    toContain(v) { check(actual.includes(v)); },
  };
}
expect.stringMatching = re => re;
const testApi = { describe(_name, run) { run(); }, expect, it(name, run) { title = name.replaceAll(upstream, "<upstream>"); cases.push({source, title}); run(); } };
function load(file) {
  file = path.resolve(file);
  if (!file.startsWith(upstream + path.sep)) throw new Error('Upstream import escaped source root');
  if (cache.has(file)) return cache.get(file).exports;
  readSources.add(path.relative(upstream,file).split(path.sep).join('/'));
  const module = { exports: {} }; cache.set(file,module);
  const localRequire = name => {
    if (name === 'bun:test') return testApi;
    if (name === 'node:fs') return { readFileSync(file, encoding) { readSources.add(path.relative(upstream,file).split(path.sep).join('/')); return fs.readFileSync(file, encoding); } };
    if (name.startsWith('node:')) return require(name);
    let target;
    if (name === '@mkagent/core') target = 'packages/core/src/index.ts';
    else if (name === '@mkagent/ui/chat/turn-utils') target = 'packages/ui/src/components/chat/turn-utils.ts';
    else if (name.startsWith('@mkagent/shared/')) target = 'packages/shared/src/' + name.slice('@mkagent/shared/'.length) + '.ts';
    else if (name.startsWith('.')) {
      target = path.resolve(path.dirname(file),name);
      if (!path.extname(target)) target += '.ts';
      return load(target);
    } else throw new Error('Unreviewed upstream runtime import: '+name);
    return load(path.join(upstream,target));
  };
  const code = ts.transpileModule(fs.readFileSync(file,'utf8'), {compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  vm.runInNewContext(`(function(require,module,exports){${code}\n})`, { Date: Clock, Math: seededMath, __dirname: path.dirname(file), console, process, Buffer, URL, structuredClone }, {filename:file})(localRequire,module,module.exports);
  if (file === path.join(processor,'turn-utils.ts') || file === path.join(processor,'message-timestamp.ts')) {
    for (const [name, original] of Object.entries(module.exports)) if (typeof original === 'function' && name !== 'storedToMessage') {
      module.exports[name] = (...args) => {
        const before = clone(args);
        const result = original(...args);
        assert.deepEqual(clone(args),before,'Upstream turns function mutated its input');
        records.push({source,title,handler:name,args:before,expected:clone(result)});
        return result;
      };
    }
  }
  return module.exports;
}
const files = fs.readdirSync(tests).filter(f=>f.endsWith('.test.ts')).sort();
assert.equal(files.length,7);
for (const file of files) { source = file; load(path.join(tests,file)); }
const originalTests = cases.length;
const originalAssertions = assertions;
const supplemental = JSON.parse(fs.readFileSync(path.join(repo,'tools/turns/scenarios.json'),'utf8'));
const utils = load(path.join(processor,'turn-utils.ts'));
for (const scenario of supplemental) {
  source = '__supplemental__'; title = scenario.title;
  for (const call of scenario.calls) {
    const args = call.args.map(v => v && typeof v === 'object' && '$number' in v ? Number(v.$number) : v);
    utils[call.handler](...args);
  }
}
const sources = Object.fromEntries([...readSources].sort().map(file=>[file,crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')]));
const artifact = JSON.stringify({upstreamCommit:pin,originalAssertions,typescript:ts.version,timezone:'UTC',randomSeed:1,localInputs:Object.fromEntries(['tools/turns/scenarios.json','tools/turns/oracle.mjs','tools/codegen/package-lock.json'].map(f=>[f,crypto.createHash('sha256').update(fs.readFileSync(path.join(repo,f))).digest('hex')])),supplemental:supplemental.map(s=>s.title),sourceGuards:['accept-plan-chevron-group.test.ts'],cases,sources,records},null,2)+'\n';
const destination = path.join(repo,'crates/agentkit-store/tests/fixtures/upstream-turns.json');
if (process.argv.includes('--check')) assert.equal(fs.readFileSync(destination,'utf8'),artifact,'Turns oracle drift');
else { fs.mkdirSync(path.dirname(destination),{recursive:true}); fs.writeFileSync(destination,artifact); }
console.log(`Original upstream assertions passed: ${originalTests} tests, ${files.length} files; ${records.length} pure function calls captured (including ${supplemental.length} extra scenarios).`);
