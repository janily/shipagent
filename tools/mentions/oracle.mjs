// Execute the pinned upstream grammar and badge extractor; only cached icons are stubbed.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {createRequire} from 'node:module';
import {execFileSync} from 'node:child_process';
const require=createRequire(import.meta.url), ts=require('../codegen/node_modules/typescript');
const repo=path.resolve(import.meta.dirname,'../..'),upstream=path.join(repo,'backend/mkagent');
const pin='6e8ebc456b6b82441be09118ca11afca79b84836';
assert.equal(execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim(),pin);
assert.equal(execFileSync('git',['status','--porcelain'],{cwd:upstream,encoding:'utf8'}).trim(),'');
const sources=['packages/shared/src/mentions/index.ts','apps/electron/src/renderer/lib/mentions.ts'];
function load(file,imports) {
 const source=fs.readFileSync(path.join(upstream,file),'utf8');
 const code=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 const module={exports:{}};
 vm.runInNewContext(`(function(require,module,exports){${code}\n})`,{},{filename:file})(name=>{assert.ok(name in imports,`Unreviewed import ${name}`);return imports[name];},module,module.exports);
 return module.exports;
}
const shared=load(sources[0],{});
const renderer=load(sources[1],{'@mkagent/shared/mentions':shared,'@mkagent/shared/skills/types':{AGENTS_PLUGIN_NAME:'.agents'},'./icon-cache':{getSkillIconSync:()=>undefined}});
const skills=[{slug:'review-code',metadata:{name:'Review',description:'Review changes'},source:'workspace',path:'/skills/review-code',content:''},{slug:'project-review',metadata:{name:'Project review',description:'Project changes'},source:'project',path:'/project/.agents/skills/project-review',content:''},{slug:'global-review',metadata:{name:'Global review',description:'Global changes'},source:'global',path:'/global/global-review',content:''}];
const texts=['plain text','[skill:review-code] [skill:project-review] [skill:global-review]','😀 [file:src/你好.rs] [folder:src] [skill:my workspace:review-code]','[skill:review-code] [skill:missing] [skill:review-code] [file:a] [file:a]','[skill:my.workspace-1_2:review-code]','[skill:bad\nworkspace:review-code] [skill:bad\tworkspace:review-code]','[skill:中文:review-code] [skill:review.code]','[file:a/] [folder:a/]','[file:[file:nested]] [folder:[file:overlap]]','[file:a\nb] [folder:src with spaces]','[file:] [folder:] [skill:]','𠮷😀[skill:.agents:project-review][file:/tmp/a]'];
const fixture={upstream:{tag:'v0.2.6',revision:pin,sources:sources.map(file=>({file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')}))},workspace:'my workspace',skills,cases:texts.map(text=>({text,parsed:shared.parseMentions(text,skills.map(s=>s.slug)),badges:renderer.extractBadges(text,skills,'my workspace')}))};
const output=path.join(repo,'crates/agentkit-store/tests/fixtures/upstream-mentions.json');
const content=JSON.stringify(fixture,null,2)+'\n';
if(process.argv.includes('--check'))assert.equal(fs.readFileSync(output,'utf8'),content,'Mention fixture drift');else fs.writeFileSync(output,content);
console.log(`Pinned upstream mentions: ${texts.length} cases, parser + renderer badge extraction`);
