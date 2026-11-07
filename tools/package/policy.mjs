// The signed job has an independent trusted-ref guard in the workflow.
import { appendFile,readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
export function selectMode({event,ref,requested='engineering',signedNightly=false}) {
  if (!['engineering','developer-id'].includes(requested)) throw new Error('Unknown signing mode');
  if (event==='pull_request') return 'engineering';
  const tag=/^refs\/tags\/v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(ref);
  if (ref!=='refs/heads/main'&&!tag) throw new Error('Packages require main or a version tag');
  if (tag||requested==='developer-id'||(event==='schedule'&&signedNightly)) return 'developer-id';
  return 'engineering';
}
export function validateTagVersion(ref,manifest){
  const section=manifest.split(/^\[/m).find(s=>s.startsWith('workspace.package]'));
  const version=section?.match(/^version\s*=\s*"([^"\r\n]+)"/m)?.[1];
  if(!version)throw new Error('Workspace version unavailable');
  if(ref?.startsWith('refs/tags/')&&ref!==`refs/tags/v${version}`)throw new Error('Version tag does not match workspace Cargo.toml');
  return version;
}
if (process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
  validateTagVersion(process.env.GITHUB_REF,await readFile(new URL('../../Cargo.toml',import.meta.url),'utf8'));
  const mode=selectMode({event:process.env.GITHUB_EVENT_NAME,ref:process.env.GITHUB_REF,requested:process.env.SHIPAGENT_PACKAGE_MODE||'engineering',signedNightly:process.env.SHIPAGENT_SIGNED_NIGHTLY==='true'});
  if(!process.env.GITHUB_OUTPUT) throw new Error('GitHub output path required');
  await appendFile(process.env.GITHUB_OUTPUT,`mode=${mode}\n`);
  console.log(`Package mode: ${mode}; engineering packages do not establish release acceptance.`);
}
