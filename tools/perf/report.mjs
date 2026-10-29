export function evaluate({native,model},expectedTokens) {
  const reasons=[];const require=(condition,reason)=>{if(!condition)reasons.push(reason);};
  require(native?.profile==='release'&&native?.optLevel==='3','release-optimized-build-required');
  require(native?.outcome==='complete'&&native?.events?.complete>=1,'agent-round-incomplete');
  require(native?.events?.tool_start>=1&&native?.events?.tool_result>=1&&model?.toolCalls>=1&&model?.toolResults>=1,'real-tool-evidence-missing');
  require(model?.errors===0,'model-errors');
  require(model?.perfStreams?.length===1,'one-complete-paced-stream-required');
  const stream=model?.perfStreams?.[0];
  require(stream?.completed===true&&stream?.tokens===expectedTokens,'token-count-incomplete');
  require(Number.isFinite(stream?.observedRate)&&stream.observedRate>=47.5&&stream.observedRate<=52.5,'input-rate-outside-50-plus-minus-5-percent');
  require(Number.isFinite(stream?.maxGapMs)&&stream.maxGapMs<=100,'bursty-model-input');
  require(stream?.bytes>0&&native?.finalAssistantBytes===stream?.bytes,'final-text-size-mismatch');
  require(native?.finalFramePresented===true,'final-frame-not-presented');
  require(Number.isFinite(native?.activeUpdateFraction)&&native.activeUpdateFraction>=.95,'native-window-not-active');
  require(Number.isFinite(native?.firstDeltaToTerminalMs)&&native.firstDeltaToTerminalMs>=stream?.elapsedMs-30,'stream-window-too-short');
  const frames=native?.frameWork?.count??0;
  require(frames>=100,'insufficient-frame-samples');
  const unavailable=(native?.unmeasuredFrames??Infinity)+(native?.unpresentedFrames??Infinity);
  require(frames/(frames+unavailable)>=.95&&(native?.unpairedPresentFrames??Infinity)<=1,'frame-measurement-coverage-below-95-percent');
  const p95=native?.frameWork?.p95_ms;
  require(Number.isFinite(p95)&&p95>0,'unmeasured-frame-work');
  return {valid:reasons.length===0,frameBudgetMet:reasons.length===0&&p95<=16,reasons,referenceAcceptance:'not_evaluated'};
}
