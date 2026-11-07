import {createHash} from 'node:crypto';
export function isolatedEnvironment(inherited,paths){
  const env={...inherited};
  for(const key of Object.keys(env))if(/TOKEN|API_KEY|PASSWORD|SECRET|CREDENTIAL/.test(key)||key.startsWith('AGENTKIT_REMOTE_')||key.startsWith('AGENTKIT_PERF_')||key==='AGENTKIT_RECORD_EVENTS')delete env[key];
  return Object.assign(env,paths);
}
export function cleanupVerdict(platform,{forced,supervision,shutdown,nativeExitCode}){
  if(platform==='linux')return {method:'linux-kernel-subreaper',forced:forced||supervision?.forcedCleanup===true,remainingOwnedProcesses:supervision?.remainingOwnedChildren??null,reapedOrphans:supervision?.reapedOrphans??null,verified:!forced&&supervision?.forcedCleanup===false&&supervision.remainingOwnedChildren===0&&supervision.nativeExitCode===0&&supervision.error===null};
  return {method:'native-owned-backend-shutdown',forced,remainingOwnedProcesses:null,backendShutdownPassed:shutdown?.backendShutdownPassed===true,verified:!forced&&nativeExitCode===0&&shutdown?.backendShutdownPassed===true};
}
export function fingerprint(environment,workload){
 const normalized={platform:environment.platform,arch:environment.arch,cpu:environment.cpu,cpuCount:environment.cpuCount,memoryBytes:environment.memoryBytes,kernel:environment.kernel,displayBackend:environment.displayBackend,vulkanDriver:environment.vulkanDriver,gpu:environment.gpu,cpuQuota:environment.cpuQuota,memoryLimit:environment.memoryLimit,machineId:environment.machineId,workload};
 return createHash('sha256').update(JSON.stringify(normalized)).digest('hex');
}
// Runtime/ownership failures are infrastructure errors, separate from a slow
// or incomplete measurement. CI may warn for the latter, never hide a crash.
export function suiteExitCode(report,streamExit){
 if(report.stream.runs.some(r=>r.error||!r.cleanup?.verified)||report.scenes.some(s=>s.status!=='measured'||!s.cleanup?.verified))return 1;
 return report.summary.valid&&!report.summary.budgetMisses.length?streamExit:2;
}
