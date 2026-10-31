import test from 'node:test';
import assert from 'node:assert/strict';
import { runProbeProcess } from './run-process.mjs';
test('probe deadline terminates a child even when SIGTERM is ignored',async()=>{
 const started=Date.now();
 const script="process.on('SIGTERM',()=>{});setTimeout(()=>process.exit(0),1500);setInterval(()=>{},20);";
 await assert.rejects(runProbeProcess(process.execPath,['-e',script],{timeout:300}),error=>error.signal==='SIGKILL');
 assert.ok(Date.now()-started<1200,'deadline did not bound probe duration');
});
