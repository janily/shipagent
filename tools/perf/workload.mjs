import { setTimeout as delay } from 'node:timers/promises';

// Deliberately simulated model tokenizer: a non-whitespace word plus trailing
// whitespace. These are not claimed to be any vendor's BPE tokens.
export const tokenize = text => text.match(/\S+\s*/gu) ?? [];
export function markdownFixture(count = 300) {
  if (!Number.isInteger(count) || count < 64 || count > 1200) throw new Error('Token count must be an integer in [64,1200]');
  const head = '# Native streaming benchmark\n\nA **fixed Markdown** fixture with a table and code.\n\n| field | value |\n| --- | --- |\n| stream | native |\n\n```rust\nfn main() {\n    println!("stream");\n}\n```\n\n';
  const remainder=count-tokenize(head).length-1;
  return head+Array.from({length:remainder},(_,i)=>`sample_${i} `).join('')+'\n\n**done**\n';
}

export async function pacedTokens(tokens, {rate=50, send, now=()=>performance.now(), wait=delay} = {}) {
  if (!Number.isFinite(rate) || rate<=0 || rate>1000 || typeof send!=='function') throw new Error('Invalid pacing options');
  const started=now(); let first, last, maxGapMs=0, bytes=0, count=0;
  for (let index=0;index<tokens.length;index++) {
    const remaining=started+index*1000/rate-now();
    if (remaining>0) await wait(remaining);
    await send(tokens[index]);
    const at=now();
    if (first===undefined) first=at;
    if (last!==undefined) maxGapMs=Math.max(maxGapMs,at-last);
    last=at; count++; bytes+=Buffer.byteLength(tokens[index]);
  }
  const elapsedMs=(last??started)-(first??started);
  return {tokens:count,bytes,elapsedMs,maxGapMs,observedRate:count>1&&elapsedMs>0?(count-1)*1000/elapsedMs:null,completed:true};
}
