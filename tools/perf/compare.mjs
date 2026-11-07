import {readFile,writeFile,appendFile} from 'node:fs/promises';
import {evaluate} from './report.mjs';
import {pathToFileURL} from 'node:url';
export const metricRules={frameWorkP95Ms:'lower',firstSubmissionMs:'lower',backendReadyMs:'lower',fixtureReadyMs:'lower',batchReceiptP95Ms:'lower',idleRssBytes:'lower',tenSessionsRssBytes:'lower',historyOpenMs:'lower',scrollFps:'higher',scrollWorkP95Ms:'lower',toolCollapsedMs:'lower',toolPreviewOpenMs:'lower',toolFullOpenMs:'lower'};
export function median(values){const sorted=values.filter(v=>Number.isFinite(v)&&v>0).sort((a,b)=>a-b);if(sorted.length!==values.length||!sorted.length)return null;const i=Math.floor(sorted.length/2);return sorted.length%2?sorted[i]:(sorted[i-1]+sorted[i])/2;}
export function scrollCoverage(scroll){
 if(!scroll||!['frames','measuredFrames','unmeasuredFrames'].every(k=>Number.isSafeInteger(scroll[k])&&scroll[k]>=0))return false;
 return scroll.frames>=120&&scroll.measuredFrames+scroll.unmeasuredFrames===scroll.frames&&scroll.measuredFrames/scroll.frames>=.95;
}
export function summarize(suite){
 const stream=suite.stream?.runs??[],scenes=suite.scenes??[];const metrics={};const put=(key,values)=>metrics[key]=median(values);
 put('frameWorkP95Ms',stream.map(r=>r.native?.frameWork?.p95_ms));put('firstSubmissionMs',stream.map(r=>r.native?.startup?.firstSubmissionMs));put('backendReadyMs',stream.map(r=>r.native?.startup?.backendReadyMs));put('fixtureReadyMs',stream.map(r=>r.native?.startup?.fixtureReadyMs));put('batchReceiptP95Ms',stream.map(r=>r.native?.batchReceiptToPresent?.p95_ms));
 for(const key of ['idleRssBytes','tenSessionsRssBytes','historyOpenMs','toolCollapsedMs','toolPreviewOpenMs','toolFullOpenMs'])put(key,scenes.map(s=>s[key]));
 put('scrollFps',scenes.map(s=>s.scroll?.fps));put('scrollWorkP95Ms',scenes.map(s=>s.scroll?.workP95Ms));
 const valid=suite.schemaVersion===2&&stream.length>=3&&scenes.length===stream.length&&stream.every(r=>r.verdict?.valid===true&&evaluate(r,suite.stream?.workload?.tokens).valid&&r.cleanup?.verified===true)&&scenes.every(s=>s.cleanup?.verified===true&&s.status==='measured'&&s.profile==='release'&&s.optLevel==='3'&&s.workload?.historyMessages===10000&&s.workload?.toolLines===100000&&s.fullEditorLines===100000&&Number.isSafeInteger(s.previewLines)&&s.previewLines>0&&s.previewLines<=2000&&scrollCoverage(s.scroll))&&Object.values(metrics).every(v=>v!==null);
 const limits={frameWorkP95Ms:16,idleRssBytes:150000000,tenSessionsRssBytes:300000000,historyOpenMs:800,scrollWorkP95Ms:16};
 const budgetMisses=valid?[...Object.entries(limits).filter(([k,limit])=>metrics[k]>limit).map(([metric,limit])=>({metric,limit,value:metrics[metric]})),...(metrics.scrollFps<60?[{metric:'scrollFps',limit:60,value:metrics.scrollFps}]:[])]:[];
 return {valid,metrics,budgetMisses,runs:stream.length,referenceAcceptance:'not_evaluated'};
}
export function compare(current,previous){
 const now=summarize(current);if(!now.valid)return {status:'invalid-current',regressions:[],current:now};
 if(!previous)return {status:'no-baseline',regressions:[],current:now};
 const before=summarize(previous);if(!before.valid)return {status:'invalid-baseline',regressions:[],current:now};
 if(current.fingerprint!==previous.fingerprint||current.methodVersion!==previous.methodVersion)return {status:'incompatible-baseline',regressions:[],current:now};
 const changes=Object.entries(metricRules).map(([key,direction])=>{const a=now.metrics[key],b=before.metrics[key];return {metric:key,current:a,baseline:b,regressionPercent:(direction==='lower'?a/b-1:1-a/b)*100};});
 const regressions=changes.filter(c=>c.regressionPercent>10+1e-8);return {status:regressions.length?'regressed':'stable',regressions,changes,current:now};
}
export function render(result){return `## Native performance: ${result.status}\n\nShared CI / configured runner metrics; reference acceptance remains not evaluated.\n\n| Metric | Median |\n| --- | ---: |\n${Object.entries(result.current.metrics).map(([k,v])=>`| ${k} | ${v??'unmeasured'} |`).join('\n')}\n\nLocal workload budget misses: ${result.current.valid?(result.current.budgetMisses.map(m=>m.metric).join(',')||'none'):'not_evaluated'}.\n\n${result.regressions.map(r=>`${r.metric}: ${r.regressionPercent.toFixed(2)}% regression`).join('\n')}\n`;}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const [currentPath,baselinePath,output]=process.argv.slice(2);if(!currentPath||!output)throw new Error('compare requires current baseline-or-dash output');
 const current=JSON.parse(await readFile(currentPath,'utf8'));let previous;if(baselinePath!=='-')previous=JSON.parse(await readFile(baselinePath,'utf8'));
 const result=compare(current,previous);await writeFile(output,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
 if(process.env.GITHUB_STEP_SUMMARY)await appendFile(process.env.GITHUB_STEP_SUMMARY,render(result));
 if(result.status!=='stable'||result.current.budgetMisses.length)console.log(`::warning title=Native performance::${result.status}; local-budget-misses=${result.current.valid?(result.current.budgetMisses.map(m=>m.metric).join(',')||'none'):'not_evaluated'}; regressions=${result.regressions.map(r=>r.metric).join(',')||'none'}; see artifact report`);
 console.log(JSON.stringify({status:result.status,metrics:result.current.metrics,referenceAcceptance:'not_evaluated'}));
}
