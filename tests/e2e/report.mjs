export const required={
 full:['u1.onboarding','u1.browser_initialized','u1.test_gate','u1.rejected_test','u1.tested','u1.saved_default','u1.credentials_private','u1.probe_cleanup','u1.home_unchanged','u1.focused_empty_session','u2.optimistic','u2.ack','u2.stream','u2.tools','u2.persisted','u3.deny','u3.allow','u3.always','u3.receipts','f17.cancel','u6.disconnected','u6.restart','u6.token_rotated','u6.session_refreshed','u6.interrupted','u6.continued'],
 restore:['restore.authenticated','restore.session','restore.browser_unchanged'],
 defer:['defer.wizard','defer.acknowledged'],
 defer_restore:['defer.restored','restore.browser_unchanged'],
};
export function evaluate(report){
 const keys=required[report?.case],reasons=[];
 if(report?.schemaVersion!==1||!keys)reasons.push('invalid-report');
 for(const key of keys??[])if(report?.checks?.[key]!==true)reasons.push(key);
 if(!Number.isSafeInteger(report?.presentedFrames)||report.presentedFrames<1)reasons.push('no-native-presentation');
 if(report?.shutdownPassed!==true)reasons.push('backend-shutdown-unverified');
 if(report?.failure)reasons.push('native-scenario-failed');
 return {passed:reasons.length===0,reasons,p0Acceptance:'incomplete',physicalAcceptance:'not_evaluated',performanceAcceptance:'not_evaluated'};
}
