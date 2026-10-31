export function verifyRound(native, model) {
 if(native?.profile!=='release'||native.optLevel!=='3'||native.outcome!=='complete'||native.finalFramePresented!==true||
    !(native.finalAssistantBytes>0)||!(native.deltaBytes>0)||!['text_delta','tool_start','tool_result','complete'].every(key=>native.events?.[key]>0)||
    !(model?.toolCalls>0)||!(model.toolResults>0)||model.errors!==0)throw new Error('Mounted native Agent round is incomplete');
}
