import { createGenerator } from 'ts-json-schema-generator';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { applyOverrides, assertInterfaceOverride, constantsRust, extractConstants, nameEvents, sorted, verifySource } from './formal.mjs';
const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--out') throw new Error('Usage: node schema.mjs --out DIR');
const output = resolve(args[1]);
const read = path => readFileSync(join(here, path), 'utf8');
const manifest = JSON.parse(read('sources.json'));
if (!Array.isArray(manifest.roots) || !manifest.roots.length || manifest.roots.some(n => typeof n !== 'string' || !/^[A-Z][A-Za-z0-9]+$/.test(n)) || new Set(manifest.roots).size !== manifest.roots.length) throw new Error('Invalid root manifest');
const allRoots=[...manifest.roots,...(manifest.schemaOnlyRoots??[])];
if (allRoots.some(n=>typeof n!=='string'||!/^[A-Z][A-Za-z0-9]+$/.test(n))||new Set(allRoots).size!==allRoots.length) throw new Error('Invalid schema-only root manifest');
const revision = execFileSync('git', ['-C', join(root, 'backend/mkagent'), 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
const gitlink = execFileSync('git', ['-C', root, 'ls-files', '--stage', 'backend/mkagent'], { encoding: 'utf8' }).split(/\s+/)[1];
const dirty = execFileSync('git', ['-C', join(root, 'backend/mkagent'), 'status', '--porcelain'], { encoding: 'utf8' });
verifySource(manifest.backendRevision, revision, gitlink, dirty);
// Extraction reads TS declarations, never executes upstream modules. Reachable
// type resolution errors still fail despite skipping unrelated application diagnostics.
const config = { path: join(here, 'input.ts'), tsconfig: join(here, 'tsconfig.json'), skipTypeCheck: true, expose: 'none', jsDoc: 'none', additionalProperties: true };
assertInterfaceOverride(readFileSync(join(root,'backend/mkagent/packages/shared/src/protocol/dto.ts'),'utf8'),read('overrides/api-input.ts'),'WorkspaceSettings');
const definitions = {};
for (const name of allRoots) {
  let schema;
  try { schema=createGenerator({ ...config, expose:['SessionFile','SkillFile'].includes(name)?'export':'none', type: name }).createSchema(name); } catch(error) {throw new Error(`Schema extraction failed for ${name}`,{cause:error});}
  if (Object.keys(schema.definitions).length !== 1 || !schema.definitions[name]) throw new Error(`Unreviewed extraction shape: ${name}`);
  definitions[name] = schema.definitions[name];
}
// Give identical nested types one canonical public name before overrides. This
// makes each override apply consistently to snapshots, options and event payloads.
const shapes = new Map(Object.entries(definitions).map(([name, shape]) => [JSON.stringify(sorted(shape)), name]));
if (shapes.size !== allRoots.length) throw new Error('Duplicate canonical root shapes');
function reuse(value, rootName, atRoot = false) {
  if (value && typeof value === 'object' && !Array.isArray(value) && !atRoot) {
    const name = shapes.get(JSON.stringify(sorted(value)));
    if (name && name !== rootName) return { $ref: `#/definitions/${name}` };
  }
  if (Array.isArray(value)) return value.map(v => reuse(v, rootName));
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k,v]) => [k, reuse(v, rootName)]));
  return value;
}
for (const name of allRoots) definitions[name] = reuse(definitions[name], name, true);
applyOverrides(definitions, JSON.parse(read('overrides/schema.json')));
nameEvents(definitions);
const upstream = 'packages/shared/src/protocol/';
const source = file => readFileSync(join(root, 'backend/mkagent', upstream, file), 'utf8');
const constants = extractConstants(source('channels.ts'), source('types.ts'), source('message-limits.ts'));
const artifacts = {};
for (const name of allRoots) {
  const reachable = {};
  function collect(key) {
    if (Object.hasOwn(reachable, key)) return;
    if (!definitions[key]) throw new Error(`Unresolved canonical type: ${key}`);
    reachable[key] = definitions[key];
    function visit(value) {
      if (!value || typeof value !== 'object') return;
      if (value.$ref) {
        if (!value.$ref.startsWith('#/definitions/')) throw new Error(`Unsupported ref: ${value.$ref}`);
        collect(value.$ref.slice('#/definitions/'.length));
      }
      for (const child of Object.values(value)) visit(child);
    }
    visit(definitions[key]);
  }
  collect(name);
  artifacts[`${name}.json`] = JSON.stringify(sorted({ $schema: 'http://json-schema.org/draft-07/schema#', $ref: `#/definitions/${name}`, $comment: `mkagent ${revision}; ts-json-schema-generator 2.9.0; effective schema with reviewed overrides`, definitions: reachable }), null, 2) + '\n';
}
artifacts['constants.rs'] = execFileSync('rustfmt', ['--edition', '2024', '--emit', 'stdout'], { input: constantsRust(constants), encoding: 'utf8' });
const digest = value => createHash('sha256').update(value).digest('hex');
artifacts['manifest.json'] = JSON.stringify(sorted({ backendRevision: revision, roots: manifest.roots, schemaOnlyRoots: manifest.schemaOnlyRoots??[], generator: 'ts-json-schema-generator 2.9.0', inputs: Object.fromEntries(['input.ts', 'sources.json', 'overrides/schema.json', 'overrides/api-input.ts', 'schema.mjs', 'formal.mjs', 'package-lock.json', 'tsconfig.json'].map(path => [path, digest(read(path))])), artifacts: Object.fromEntries(Object.entries(artifacts).map(([name, bytes]) => [name, digest(bytes)])) }), null, 2) + '\n';
// Do not emit partial files on extraction/override failure.
mkdirSync(output, { recursive: true });
for (const [name, bytes] of Object.entries(artifacts)) writeFileSync(join(output, name), bytes);
