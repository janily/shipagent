import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, symlink, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { stageBundle, nativeFiles, validateOptions, signPlan } from './bundle.mjs';
const brand={app_name:'Workbench Test',bundle_id:'dev.agentkit.workbench',url_scheme:'agentkit'};
const macho=Buffer.from('cffaedfe0c000001','hex');
async function fixture(t) {
 const root=await mkdtemp(join(tmpdir(),'shipagent-package-test-')); t.after(()=>rm(root,{recursive:true,force:true}));
 const backend=join(root,'backend');
 for(const [name,bytes] of [['server.js','server'],['resources/pi-agent-server/index.js','pi'],['vendor/bun/bun',macho],['node_modules/@vscode/ripgrep/bin/rg',macho],['resources/bin/darwin-arm64/uv',macho],['LICENSE','license'],['NOTICE','notice']]) {
  await mkdir(join(backend,name,'..'),{recursive:true}); await writeFile(join(backend,name),bytes);
 }
 const binary=join(root,'workbench');await writeFile(binary,macho);
 const license=join(root,'LICENSE');await writeFile(license,'application license');
 return {root,backend,binary,license,brand,version:'0.1.0',target:'macos-arm64',output:join(root,'Workbench Test.app')};
}
test('bundle layout, branding, licenses and extensionless Mach-O inventory',async t=>{
 const f=await fixture(t);await stageBundle(f);
 const plist=await readFile(join(f.output,'Contents/Info.plist'),'utf8');
 assert.match(plist,/<key>CFBundleExecutable<\/key>\s*<string>workbench<\/string>/);
 assert.match(plist,/<string>dev.agentkit.workbench<\/string>/);
 assert.equal(await readFile(join(f.output,'Contents/Resources/backend/server.js'),'utf8'),'server');
 assert.equal(await readFile(join(f.output,'Contents/Resources/LICENSE'),'utf8'),'application license');
 // POSIX executable bits are verified on Linux/macOS; Windows has no such bits.
 if(process.platform!=='win32')assert.equal((await stat(join(f.output,'Contents/MacOS/workbench'))).mode&0o111,0o111);
 assert.deepEqual((await nativeFiles(f.output)).map(x=>x.replaceAll('\\','/')).sort(),[
 'Contents/MacOS/workbench','Contents/Resources/backend/node_modules/@vscode/ripgrep/bin/rg','Contents/Resources/backend/resources/bin/darwin-arm64/uv','Contents/Resources/backend/vendor/bun/bun'].sort());
 const plan=signPlan(await nativeFiles(f.output));
 assert.equal(plan.filter(x=>x.bun).length,1);
 assert.equal(plan.at(-1).app,true);
});
test('invalid brands, sign modes and notarizing ad-hoc fail validation',()=>{
 for(const name of ['../Escape','/tmp/Escape','.', 'Bad\nName']) assert.throws(()=>validateOptions({brand:{...brand,app_name:name},version:'0.1.0',target:'macos-arm64',sign:'ad-hoc'}));
 assert.throws(()=>validateOptions({brand,version:'0.1.0',target:'macos-arm64',sign:'unsigned'}));
 assert.throws(()=>validateOptions({brand,version:'0.1.0',target:'macos-arm64',sign:'ad-hoc',notarize:true}));
});
test('existing output is preserved; missing backend, WebUI and escaping links rejected',async t=>{
 const f=await fixture(t);await mkdir(f.output);await writeFile(join(f.output,'sentinel'),'keep');
 await assert.rejects(stageBundle(f));assert.equal(await readFile(join(f.output,'sentinel'),'utf8'),'keep');
 await rm(f.output,{recursive:true});await mkdir(join(f.backend,'webui'));await assert.rejects(stageBundle(f),/WebUI/);
 await rm(join(f.backend,'webui'),{recursive:true});await symlink(f.license,join(f.backend,'external-license'));await assert.rejects(stageBundle(f),/symlink/);
 await rm(join(f.backend,'external-license'));await rm(join(f.backend,'server.js'));await assert.rejects(stageBundle(f),/server.js/);
});
test('JIT success requires observed compilation, not just JS output', async()=>{
 const { parseJitTrial }=await import('./bundle.mjs');
 assert.deepEqual(parseJitTrial('{"marker":"SHIPAGENT_JIT_OK","dfgCompiles":2,"wasmResult":7}'),{passed:true,dfgCompiles:2,wasmResult:7});
 assert.throws(()=>parseJitTrial('{"marker":"SHIPAGENT_JIT_OK","dfgCompiles":0,"wasmResult":7}'));
 assert.throws(()=>parseJitTrial('SHIPAGENT_JIT_OK'));
});
test('Java bytecode sharing FAT magic is not treated as Mach-O',async t=>{
 const f=await fixture(t);await mkdir(f.output);
 await writeFile(join(f.output,'Example.class'),Buffer.from('cafebabe00000034001007000201000454657374','hex'));
 assert.deepEqual(await nativeFiles(f.output),[]);
});
test('hardened-runtime evidence must come from CodeDirectory flags',async()=>{
 const { requireRuntimeFlags }=await import('./bundle.mjs');
 assert.equal(requireRuntimeFlags('CodeDirectory v=20500 size=1 flags=0x10002(adhoc,runtime)'),0x10002);
 assert.throws(()=>requireRuntimeFlags('CodeDirectory v=20500 size=1 flags=0x2(adhoc)'));
 assert.throws(()=>requireRuntimeFlags('Executable=/tmp/flags=0x10000\nCodeDirectory v=20500 flags=0x0(none)'));
});
