import test from 'node:test';
import assert from 'node:assert/strict';
import {selectMode,validateTagVersion} from './policy.mjs';
test('pull requests never select credentials, even when signing is requested',()=>{
  assert.equal(selectMode({event:'pull_request',ref:'refs/pull/29/merge',requested:'developer-id',signedNightly:true}),'engineering');
});
test('nightly without explicit opt-in preserves engineering artifacts',()=>{
  assert.equal(selectMode({event:'schedule',ref:'refs/heads/main'}),'engineering');
  assert.equal(selectMode({event:'schedule',ref:'refs/heads/main',signedNightly:true}),'developer-id');
  assert.equal(selectMode({event:'workflow_dispatch',ref:'refs/heads/main',requested:'developer-id'}),'developer-id');
});
test('version tags require signing, and arbitrary refs/modes fail closed',()=>{
  assert.equal(selectMode({event:'push',ref:'refs/tags/v0.1.0-beta.1'}),'developer-id');
  for(const ref of ['refs/heads/feature','refs/tags/nightly','refs/tags/vbad'])assert.throws(()=>selectMode({event:'workflow_dispatch',ref}));
  assert.throws(()=>selectMode({event:'schedule',ref:'refs/heads/main',requested:'auto-fallback'}));
});

test('a version tag must name the Cargo workspace version before signing can start',()=>{
 const manifest='[workspace.package]\nversion = "0.1.0-beta.1"\n[dependencies]\nother = "1"\n';
 assert.equal(validateTagVersion('refs/tags/v0.1.0-beta.1',manifest),'0.1.0-beta.1');
 assert.throws(()=>validateTagVersion('refs/tags/v9.9.9',manifest),/does not match/);
 assert.throws(()=>validateTagVersion('refs/heads/main','[package]\nversion="1.0.0"'),/unavailable/);
});
