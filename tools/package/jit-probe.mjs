// Run by the signed Bun candidate, never by Node. Pin Bun alongside backend.
import { optimizeNextInvocation, numberOfDFGCompiles } from 'bun:jsc';
const add=new Function('a','b','return a+b');
let sum=0;for(let i=0;i<100000;i++)sum=add(sum,i);
if(sum!==4999950000)throw new Error('Incorrect JS result');
optimizeNextInvocation(add);
for(let batch=0;batch<100&&numberOfDFGCompiles(add)<1;batch++){
 for(let i=0;i<100000;i++)add(i,1);
 await new Promise(resolve=>setTimeout(resolve,10));
}
const dfgCompiles=numberOfDFGCompiles(add);
if(dfgCompiles<1)throw new Error('No observed optimizing JIT compilation');
const wasm=new Uint8Array([0,97,115,109,1,0,0,0,1,7,1,96,2,127,127,1,127,3,2,1,0,7,7,1,3,97,100,100,0,0,10,9,1,7,0,32,0,32,1,106,11]);
const {instance}=await WebAssembly.instantiate(wasm);const wasmResult=instance.exports.add(3,4);
if(wasmResult!==7)throw new Error('Incorrect Wasm result');
console.log(JSON.stringify({marker:'SHIPAGENT_JIT_OK',dfgCompiles,wasmResult}));
