import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const exec=promisify(execFile);
export function runProbeProcess(binary,args,options={}) {
 return exec(binary,args,{timeout:65_000,maxBuffer:1024*1024,...options,killSignal:'SIGKILL'});
}
