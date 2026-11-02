import ts from 'typescript';
import { isDeepStrictEqual } from 'node:util';

export function sorted(value) {
  if (Array.isArray(value)) return value.map(sorted);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([k, v]) => [k, sorted(v)]));
  return value;
}

export function verifySource(expected, revision, gitlink, dirty) {
  if (expected !== revision) throw new Error('Backend revision differs from sources.json; review upstream upgrade');
  if (expected !== gitlink) throw new Error('Backend gitlink differs from sources.json; stage the reviewed submodule pin');
  if (dirty.trim()) throw new Error('Backend source is dirty; generate only from the reviewed commit');
}

export function applyOverrides(definitions, entries) {
  const seen = new Set();
  for (const { pointer, from, to, reason } of entries) {
    if (!reason?.trim() || !pointer?.startsWith('/')) throw new Error('Override requires a JSON pointer and reason');
    if (seen.has(pointer)) throw new Error(`Duplicate override: ${pointer}`);
    seen.add(pointer);
    const parts = pointer.slice(1).split('/').map(v => v.replaceAll('~1', '/').replaceAll('~0', '~'));
    let parent = definitions;
    for (const part of parts.slice(0, -1)) {
      if (!parent || !Object.hasOwn(parent, part)) throw new Error(`Missing override path: ${pointer}`);
      parent = parent[part];
    }
    const key = parts.at(-1);
    if (!parent || !Object.hasOwn(parent, key)) throw new Error(`Missing override path: ${pointer}`);
    if (!isDeepStrictEqual(parent[key], from)) throw new Error(`Override source changed: ${pointer}`);
    parent[key] = structuredClone(to);
  }
  return definitions;
}

const pascal = value => value.split('_').map(s => s[0].toUpperCase() + s.slice(1)).join('');
export function nameEvents(definitions) {
  const union = definitions.SessionEvent;
  if (!Array.isArray(union?.anyOf)) throw new Error('Expected SessionEvent anyOf');
  const names = new Set();
  union.anyOf = union.anyOf.map(shape => {
    const tag = shape.properties?.type?.const;
    if (typeof tag !== 'string' || !/^[a-z][a-z_]*$/.test(tag)) throw new Error('Event requires a literal type discriminator');
    let suffix = '';
    if (tag === 'retry') {
      const phase = shape.properties.phase;
      if (isDeepStrictEqual(phase, { const: 'backoff', type: 'string' }) || isDeepStrictEqual(phase, { const: 'backoff' })) suffix = 'Backoff';
      else if (isDeepStrictEqual(phase, { enum: ['active', 'end'], type: 'string' })) suffix = 'State';
      else throw new Error('Unreviewed retry shape');
    }
    const name = `Session${pascal(tag)}${suffix}Event`;
    if (names.has(name) || Object.hasOwn(definitions, name)) throw new Error(`Duplicate event shape: ${name}`);
    names.add(name);
    definitions[name] = shape;
    return { $ref: `#/definitions/${name}` };
  }).sort((a, b) => a.$ref < b.$ref ? -1 : 1);
}

function declarations(source) {
  const file = ts.createSourceFile('input.ts', source, ts.ScriptTarget.Latest, true);
  if (file.parseDiagnostics.length) throw new Error('Invalid TypeScript constants input');
  const result = new Map();
  for (const statement of file.statements) {
    if (!ts.isVariableStatement(statement) || !statement.modifiers?.some(m => m.kind === ts.SyntaxKind.ExportKeyword)) continue;
    for (const decl of statement.declarationList.declarations) {
      if (!ts.isIdentifier(decl.name) || !decl.initializer) throw new Error('Unsupported constant declaration');
      if (result.has(decl.name.text)) throw new Error(`Duplicate constant: ${decl.name.text}`);
      result.set(decl.name.text, decl.initializer);
    }
  }
  return result;
}

// Deliberately supports a tiny expression language, never eval/import upstream.
function literal(node, env) {
  if (ts.isAsExpression(node) || ts.isParenthesizedExpression(node)) return literal(node.expression, env);
  if (ts.isStringLiteral(node)) return node.text;
  if (ts.isNumericLiteral(node)) return Number(node.text);
  if (ts.isIdentifier(node) && env.has(node.text)) return env.get(node.text);
  if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.AsteriskToken) {
    const left = literal(node.left, env), right = literal(node.right, env);
    if (typeof left === 'number' && typeof right === 'number') return left * right;
  }
  if (ts.isObjectLiteralExpression(node)) {
    const result = Object.create(null);
    for (const property of node.properties) {
      if (!ts.isPropertyAssignment(property) || (!ts.isIdentifier(property.name) && !ts.isStringLiteral(property.name))) throw new Error('Unsupported object literal property');
      const key = property.name.text;
      if (Object.hasOwn(result, key)) throw new Error(`Duplicate literal key: ${key}`);
      result[key] = literal(property.initializer, env);
    }
    return result;
  }
  throw new Error('Unsupported literal expression in protocol constants');
}

export function extractConstants(channelsSource, typesSource, limitsSource) {
  const channelsNode = declarations(channelsSource).get('RPC_CHANNELS');
  const versionNode = declarations(typesSource).get('PROTOCOL_VERSION');
  if (!channelsNode || !versionNode) throw new Error('Missing protocol constants');
  const channels = literal(channelsNode, new Map());
  const protocolVersion = literal(versionNode, new Map());
  if (!channels || typeof channels !== 'object' || typeof protocolVersion !== 'string') throw new Error('Invalid protocol constant shape');
  const limits = new Map();
  for (const [key, node] of declarations(limitsSource)) {
    const value = literal(node, limits);
    if (!Number.isSafeInteger(value) || value < 0) throw new Error(`Unsupported non-integer limit: ${key}`);
    limits.set(key, value);
  }
  return { channels, protocolVersion, limits: Object.fromEntries(limits) };
}

export function constantsRust({ channels, protocolVersion, limits }) {
  const valid = (name) => {
    if (!/^[a-zA-Z_][a-zA-Z_0-9]*$/.test(name)) throw new Error(`Invalid Rust identifier: ${name}`);
    // Rust keyword namespaces use raw identifiers; these four cannot be raw.
    if (['self', 'Self', 'super', 'crate'].includes(name)) throw new Error(`Reserved Rust identifier: ${name}`);
    return `r#${name}`;
  };
  let output = '// @generated by cargo xtask codegen; do not edit.\n';
  output += `pub const PROTOCOL_VERSION: &str = ${JSON.stringify(protocolVersion)};\n`;
  output += 'pub mod channels {\n';
  const all = [];
  const moduleNames = new Set();
  for (const [group, entries] of Object.entries(sorted(channels))) {
    const moduleName = group.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
    if (moduleNames.has(moduleName)) throw new Error(`Duplicate Rust namespace: ${moduleName}`);
    moduleNames.add(moduleName);
    if (!entries || typeof entries !== 'object') throw new Error(`Invalid channel namespace: ${group}`);
    output += `pub mod ${valid(moduleName)} {\n`;
    for (const [key, value] of Object.entries(entries)) {
      if (typeof value !== 'string' || !/^[\x20-\x7e]+$/.test(value)) throw new Error(`Invalid channel: ${group}.${key}`);
      output += `pub const ${valid(key)}: &str = ${JSON.stringify(value)};\n`;
      all.push(value);
    }
    output += '}\n';
  }
  if (!all.length) throw new Error('Empty RPC_CHANNELS');
  output += `pub const ALL: &[&str] = &${JSON.stringify([...new Set(all)].sort())};\n}\n`;
  output += 'pub mod limits {\n';
  for (const [key, value] of Object.entries(sorted(limits))) output += `pub const ${valid(key)}: u64 = ${value};\n`;
  return output + '}\n';
}

// This rewrite only replaces the exact unsupported inline import expression.
// Any field addition/removal/type/optionality change requires a fresh review.
export function assertInterfaceOverride(upstream, override, name) {
  function fields(source, original) {
    const file=ts.createSourceFile('types.ts',source,ts.ScriptTarget.Latest,true);
    const node=file.statements.find(n=>ts.isInterfaceDeclaration(n)&&n.name.text===name);
    if(!node) throw new Error(`Missing override interface ${name}`);
    if(node.heritageClauses?.length)throw new Error(`Unsupported override heritage: ${name}`);
    return node.members.map(member=>{
      if(!ts.isPropertySignature(member)||!member.type) throw new Error('Unsupported override field');
      let type=member.type.getText(file);
      if(original) type=type.replaceAll("import('../workspaces/types').ChatStarter",'ChatStarter');
      return `${member.name.getText(file)}${member.questionToken?'?':''}:${type.replace(/\s/g,'')}`;
    }).sort();
  }
  if(!isDeepStrictEqual(fields(upstream,true),fields(override,false)))throw new Error(`Interface override source changed: ${name}`);
}
