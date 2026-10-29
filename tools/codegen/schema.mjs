import { createGenerator } from 'ts-json-schema-generator';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--out') throw new Error('Usage: node schema.mjs --out DIR');
const output = resolve(args[1]);
// Type extraction only: no upstream application dependencies or code are executed.
// The generator still fails on unresolved reachable types; skipTypeCheck avoids
// unrelated diagnostics from runtime imports in upstream public barrels.
const config = { path: join(here, 'input.ts'), tsconfig: join(here, 'tsconfig.json'), skipTypeCheck: true, expose: 'none', jsDoc: 'none', additionalProperties: true };
const revision = execFileSync('git', ['-C', join(root, 'backend/mkagent'), 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
function sorted(value) {
  if (Array.isArray(value)) return value.map(sorted);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).sort(([a],[b]) => a.localeCompare(b, 'en')).map(([k,v]) => [k,sorted(v)]));
  return value;
}
mkdirSync(output, { recursive: true });
const schemas = Object.fromEntries(['Message','Session','SessionEvent'].map(name => [name, createGenerator({ ...config, type: name }).createSchema(name)]));
const message = schemas.Message.definitions.Message;
const messageKey = JSON.stringify(sorted(message));
function reuseMessage(value) {
  if (value && typeof value === 'object' && !Array.isArray(value) && JSON.stringify(sorted(value)) === messageKey) return { $ref: '#/definitions/Message' };
  if (Array.isArray(value)) return value.map(reuseMessage);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k,v]) => [k,reuseMessage(v)]));
  return value;
}
for (const [name, extracted] of Object.entries(schemas)) {
  const schema = name === 'Message' ? extracted : reuseMessage(extracted);
  if (name !== 'Message') schema.definitions.Message = message;
  schema.$comment = `mkagent ${revision}; ts-json-schema-generator 2.9.0; tools/codegen/input.ts`;
  writeFileSync(join(output, `${name}.json`), JSON.stringify(sorted(schema), null, 2)+'\n');
}
