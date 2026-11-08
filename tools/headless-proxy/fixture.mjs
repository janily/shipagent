// Local-only proof endpoints. Never contacts a real model or forwards arbitrary targets.
import http from 'node:http';import {writeFileSync} from 'node:fs';
const file=process.argv[2];if(!file)throw Error('Private counter path required');
const counts={origin:0,a:0,b:0};const sockets=new Set();
const body=JSON.stringify({content:[{type:'text',text:'fixture-ok'}]});
const save=()=>writeFileSync(file,JSON.stringify(counts));save();
function server(kind){const s=http.createServer((req,res)=>{counts[kind]++;save();req.resume();res.writeHead(200,{'content-type':'application/json'});res.end(body)});s.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket))});return s;}
const origin=server('origin'),a=server('a'),b=server('b');
for(const s of [origin,a,b])await new Promise(resolve=>s.listen(0,'127.0.0.1',resolve));
const url=s=>`http://127.0.0.1:${s.address().port}`;
console.log(JSON.stringify({origin:url(origin),a:url(a),b:url(b)}));
function stop(){for(const socket of sockets)socket.destroy();for(const s of [origin,a,b])s.close();}
process.once('SIGTERM',stop);process.once('SIGINT',stop);
