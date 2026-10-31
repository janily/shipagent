// Public verification metadata only; no configs, credentials or transcripts.
import { readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
const output=resolve(process.argv[2]??'target/packages/macos-arm64');
const pkg=JSON.parse(await readFile(join(output,'package-report.json'),'utf8'));
const mounted=JSON.parse(await readFile(join(output,'mounted-verification.json'),'utf8'));
const summary={
 schemaVersion:1,ciRun:process.env.GITHUB_RUN_ID??null,sourceCommit:process.env.GITHUB_SHA??null,
 version:pkg.version,target:pkg.target,signMode:pkg.signMode,hardenedRuntime:pkg.hardenedRuntime,probeBuild:pkg.probeBuild,
 notarization:pkg.notarization,gatekeeperAcceptance:pkg.gatekeeperAcceptance,cleanMachineAcceptance:pkg.cleanMachineAcceptance,
 appLogicalBytes:pkg.appLogicalBytes,dmg:pkg.dmg,bunPolicy:pkg.bunPolicy,bunTrials:pkg.bunTrials,
 signedNativeFiles:pkg.nativeFiles.length,runtimeFlags:pkg.nativeFiles.map(file=>({path:file.path,flags:file.codeDirectoryFlags})),bunEntitlements:pkg.nativeFiles.find(file=>file.bun)?.entitlementKeys,
 nonBunEntitlementsEmpty:pkg.nativeFiles.filter(file=>!file.bun).every(file=>file.entitlementKeys?.length===0),
 mounted:{status:mounted.status,sourceBackendUnavailable:mounted.sourceBackendUnavailable,backendOverrideUsed:mounted.backendOverrideUsed,backendContract:mounted.backendContract,sourceBackendRestored:mounted.sourceBackendRestored,dmgDetached:mounted.dmgDetached,
 native:mounted.native?{profile:mounted.native.profile,optLevel:mounted.native.optLevel,outcome:mounted.native.outcome,events:mounted.native.events,finalAssistantBytes:mounted.native.finalAssistantBytes,finalFramePresented:mounted.native.finalFramePresented}:null},
};
await writeFile(join(output,'verification-summary.json'),JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(summary));
if(mounted.status!=='passed')process.exitCode=1;
