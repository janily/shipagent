// CI can restore package outputs with Rust's target cache. Preserve them first;
// the packager itself must continue to refuse overwriting any existing output.
import {lstat,mkdir,mkdtemp,rename} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
export async function preserveCachedOutput(output,archiveRoot){
 let info;try{info=await lstat(output);}catch(error){if(error.code==='ENOENT')return null;throw error;}
 if(!info.isDirectory()||info.isSymbolicLink())throw new Error('Cached package output must be a directory; preserve unexpected output manually');
 await mkdir(archiveRoot,{recursive:true});const archive=join(await mkdtemp(join(archiveRoot,'shipagent-cached-package-')),'package');
 await rename(output,archive);return archive;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const [target]=process.argv.slice(2);if(!['macos-arm64','macos-x64'].includes(target)||!process.env.RUNNER_TEMP)throw new Error('Explicit macOS target and RUNNER_TEMP required');
 const archive=await preserveCachedOutput(resolve('target/packages',target),process.env.RUNNER_TEMP);console.log(JSON.stringify({preservedCachedPackage:archive!==null}));
}
