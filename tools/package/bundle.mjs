import { cp, mkdir, readFile, writeFile, chmod, readdir, realpath, lstat, open } from 'node:fs/promises';
import { join, dirname, relative, sep, resolve } from 'node:path';
export function validateOptions({brand,version,target,sign,notarize=false}) {
 if(!brand || !/^[\p{L}\p{N}][\p{L}\p{N} ._-]{0,63}$/u.test(brand.app_name) || brand.app_name.trim()!==brand.app_name ||
    !/^[A-Za-z][A-Za-z0-9-]*(\.[A-Za-z][A-Za-z0-9-]*)+$/.test(brand.bundle_id) ||
    !/^[a-z][a-z0-9+.-]*$/.test(brand.url_scheme) || !/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(version) ||
    !['macos-arm64','macos-x64'].includes(target) || !['ad-hoc','developer-id'].includes(sign) || (notarize&&sign!=='developer-id')) throw new Error('Invalid packaging options/brand/signing mode');
}
const xml=s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
function plist(brand,version) {
 const values={CFBundleName:brand.app_name,CFBundleDisplayName:brand.app_name,CFBundleIdentifier:brand.bundle_id,CFBundleExecutable:'workbench',CFBundlePackageType:'APPL',CFBundleShortVersionString:version.split('-')[0],CFBundleVersion:version.split('-')[0],ShipAgentVersion:version,LSMinimumSystemVersion:'13.0',NSHighResolutionCapable:true};
 return `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict>\n${Object.entries(values).map(([key,value])=>`<key>${key}</key>${value===true?'<true/>':`<string>${xml(value)}</string>`}`).join('\n')}\n<key>CFBundleURLTypes</key><array><dict><key>CFBundleURLSchemes</key><array><string>${xml(brand.url_scheme)}</string></array></dict></array>\n</dict></plist>\n`;
}
async function validateTree(root,path=root,ancestors=new Set()) {
 const actual=await realpath(path);const rel=relative(root,actual);
 if(rel==='..'||rel.startsWith(`..${sep}`)||resolve(actual)===dirname(root))throw new Error('Escaping backend symlink');
 const info=await lstat(actual);
 if(info.isDirectory()) {
  if(ancestors.has(actual))throw new Error('Cyclic backend symlink');
  for(const item of await readdir(actual))await validateTree(root,join(actual,item),new Set([...ancestors,actual]));
 }
}
export async function stageBundle({brand,version,target,backend,binary,license,output}) {
 validateOptions({brand,version,target,sign:'ad-hoc'});
 try {await lstat(output);throw new Error('Bundle output already exists');}catch(error){if(error.code!=='ENOENT')throw error;}
 try {await lstat(join(backend,'webui'));throw new Error('WebUI must not be bundled');}catch(error){if(error.code!=='ENOENT')throw error;}
 for(const name of ['server.js','resources/pi-agent-server/index.js','vendor/bun/bun','node_modules/@vscode/ripgrep/bin/rg',`resources/bin/darwin-${target.endsWith('arm64')?'arm64':'x64'}/uv`,'LICENSE','NOTICE']) {
  const info=await lstat(join(backend,name));if(!info.isFile())throw new Error(`Missing backend artifact: ${name}`);
 }
 await validateTree(await realpath(backend));
 await mkdir(join(output,'Contents/MacOS'),{recursive:true});await mkdir(join(output,'Contents/Resources'));
 await cp(binary,join(output,'Contents/MacOS/workbench'));await chmod(join(output,'Contents/MacOS/workbench'),0o755);
 await cp(backend,join(output,'Contents/Resources/backend'),{recursive:true,dereference:true});
 await cp(license,join(output,'Contents/Resources/LICENSE'));
 await writeFile(join(output,'Contents/Info.plist'),plist(brand,version));
 await writeFile(join(output,'Contents/PkgInfo'),'APPL????');
}
export async function nativeFiles(root) {
 const result=[];
 async function visit(path) {
  const info=await lstat(path);
  if(info.isSymbolicLink())throw new Error('Unexpected bundle symlink');
  if(info.isDirectory()){for(const name of await readdir(path))await visit(join(path,name));return;}
  if(!info.isFile()||info.size<4)return;
  const handle=await open(path,'r');const bytes=Buffer.alloc(32);
  try{await handle.read(bytes,0,32,0);}finally{await handle.close();}
  const magic=bytes.subarray(0,4).toString('hex');
  const thin=['feedface','feedfacf','cefaedfe','cffaedfe'].includes(magic);
  let fat=false;
  if(['cafebabe','bebafeca','cafebabf','bfbafeca'].includes(magic)){
   const little=['bebafeca','bfbafeca'].includes(magic);
   const count=little?bytes.readUInt32LE(4):bytes.readUInt32BE(4);
   const cpu=little?bytes.readUInt32LE(8):bytes.readUInt32BE(8);
   const entryBytes=['cafebabf','bfbafeca'].includes(magic)?32:20;
   fat=count>0&&count<=64&&info.size>=8+count*entryBytes&&[7,12,18,0x01000007,0x0100000c,0x01000012].includes(cpu);
  }
  if(thin||fat)result.push(relative(root,path));
 }
 await visit(root);return result.sort();
}
export function signPlan(files) {
 return [...files].sort((a,b)=>b.split(sep).length-a.split(sep).length||a.localeCompare(b)).map(path=>({path,bun:path.split(sep).join('/')==='Contents/Resources/backend/vendor/bun/bun'})).concat([{path:'.',app:true,bun:false}]);
}

export function parseJitTrial(stdout) {
 const result=JSON.parse(stdout);
 if(result.marker!=='SHIPAGENT_JIT_OK'||!Number.isSafeInteger(result.dfgCompiles)||result.dfgCompiles<1||result.wasmResult!==7)throw new Error('No JIT/Wasm evidence');
 return {passed:true,dfgCompiles:result.dfgCompiles,wasmResult:result.wasmResult};
}
