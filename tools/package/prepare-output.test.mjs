import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,lstat,rm,symlink} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {preserveCachedOutput} from './prepare-output.mjs';
test('restored package output is preserved and no longer blocks a fresh protected build',async t=>{
 const root=await mkdtemp(join(tmpdir(),'package-cache-test-'));t.after(()=>rm(root,{recursive:true,force:true}));const output=join(root,'target/packages/macos-arm64');await mkdir(output,{recursive:true});await writeFile(join(output,'old.dmg'),'preserve exact artifact');
 const archived=await preserveCachedOutput(output,join(root,'runner-temp'));
 await assert.rejects(lstat(output),{code:'ENOENT'},'cached output still blocks packaging');assert.equal(await readFile(join(archived,'old.dmg'),'utf8'),'preserve exact artifact');
 await mkdir(output);await writeFile(join(output,'new.dmg'),'new artifact');assert.equal(await readFile(join(archived,'old.dmg'),'utf8'),'preserve exact artifact');
});
test('a fresh output needs no archive; unexpected files and symlinks are not moved',async t=>{
 const root=await mkdtemp(join(tmpdir(),'package-cache-test-'));t.after(()=>rm(root,{recursive:true,force:true}));assert.equal(await preserveCachedOutput(join(root,'absent'),root),null);
 const file=join(root,'file');await writeFile(file,'preserve');await assert.rejects(preserveCachedOutput(file,root));assert.equal(await readFile(file,'utf8'),'preserve');
 if(process.platform!=='win32'){const link=join(root,'link');await symlink(file,link);await assert.rejects(preserveCachedOutput(link,root));assert.equal((await lstat(link)).isSymbolicLink(),true);}
});
