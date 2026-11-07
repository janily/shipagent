import {mkdir,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {pathToFileURL} from 'node:url';
const exec=promisify(execFile);
export function selectPrevious(runs,current){
 return runs.filter(r=>r.id!==current.id&&r.head_branch==='main'&&r.event==='schedule'&&Date.parse(r.created_at)<Date.parse(current.created_at)).sort((a,b)=>Date.parse(b.created_at)-Date.parse(a.created_at))[0]??null;
}
export async function downloadBaseline(env,request=fetch){
 const repo=env.GITHUB_REPOSITORY,name=env.PERF_ARTIFACT_NAME,out=resolve(env.PERF_BASELINE_DIR||'target/perf/baseline');
 if(!/^[\w.-]+\/[\w.-]+$/.test(repo||'')||!/^perf-[\w-]+$/.test(name||'')||!/^\d+$/.test(env.GITHUB_RUN_ID||''))throw new Error('Invalid baseline repository/run/name');
 await mkdir(out,{recursive:true});let status={status:'no-baseline'};
 async function api(path){const r=await request(`https://api.github.com/repos/${repo}/${path}`,{signal:AbortSignal.timeout(15000),headers:{Accept:'application/vnd.github+json',Authorization:`Bearer ${env.GITHUB_TOKEN}`}});if(!r.ok)throw new Error('Baseline API unavailable');return r.json();}
 try{
  const current=await api(`actions/runs/${env.GITHUB_RUN_ID}`);
  const history=await api(`actions/workflows/performance.yml/runs?branch=main&event=schedule&per_page=30`);
  const previous=selectPrevious(history.workflow_runs,current);
  if(previous){status={status:'previous-run-not-complete',runId:previous.id};
   if(previous.status==='completed'){
    const artifacts=await api(`actions/runs/${previous.id}/artifacts?name=${encodeURIComponent(name)}`);const artifact=artifacts.artifacts?.find(a=>a.name===name&&!a.expired);
    status={status:'previous-run-missing-artifact',runId:previous.id};
    if(artifact){
     if(artifact.size_in_bytes>8*1024*1024)throw new Error('Baseline artifact too large');
     const response=await request(`https://api.github.com/repos/${repo}/actions/artifacts/${artifact.id}/zip`,{signal:AbortSignal.timeout(60000),headers:{Authorization:`Bearer ${env.GITHUB_TOKEN}`}});if(!response.ok)throw new Error('Baseline archive unavailable');
     const bytes=Buffer.from(await response.arrayBuffer());if(bytes.length>8*1024*1024)throw new Error('Baseline archive too large');
     const zip=join(out,'baseline.zip');await writeFile(zip,bytes,{flag:'wx',mode:0o600});
     await exec(process.platform==='win32'?'python':'python3',['-c',"import sys,zipfile,json;z=zipfile.ZipFile(sys.argv[1]);matches=[i for i in z.infolist() if i.filename in ('report.json','perf/report.json')];assert len(matches)==1;i=matches[0];assert i.file_size<=8*1024*1024;data=z.read(i);json.loads(data);open(sys.argv[2],'xb').write(data)",zip,join(out,'report.json')],{timeout:10000,maxBuffer:1024});
     status={status:'downloaded',runId:previous.id,artifactId:artifact.id};
    }
   }
  }
 }catch{status={...status,status:'baseline-unavailable'};}
 await writeFile(join(out,'baseline-status.json'),JSON.stringify(status,null,2)+'\n',{flag:'wx'});return status;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href)console.log(JSON.stringify(await downloadBaseline(process.env)));
