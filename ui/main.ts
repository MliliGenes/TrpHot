import { createCliRenderer } from '@opentui/core';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mount } from './app';
const binary = process.env.TRPHOT_ENGINE;
if(!binary) throw new Error('Launch this interface through trphot.');
const renderer = await createCliRenderer({exitOnCtrlC:false, targetFps:20});
const child=spawn(binary,[...process.argv.slice(2),'--engine'],{stdio:['pipe','pipe','pipe']});
let ending=false;
function quit(){if(ending)return;ending=true;child.stdin.end(JSON.stringify({cmd:'quit'})+'\n');setTimeout(()=>child.kill('SIGTERM'),2000).unref();}
const app=mount(renderer,v=>{if(!ending && child.stdin.writable)child.stdin.write(JSON.stringify(v)+'\n');},quit);
createInterface({input:child.stdout}).on('line',s=>{try{app.receive(JSON.parse(s));}catch{app.fail('Invalid engine event');}});
createInterface({input:child.stderr}).on('line',s=>app.fail(s));
child.stdin.on('error',()=>{});
child.on('error',e=>{renderer.destroy();console.error(e.message);process.exitCode=1;});
child.on('exit',(code)=>{renderer.destroy();if(code)console.error(`Rust engine exited with code ${code}`);process.exitCode=code??1;});
for(const signal of ['SIGINT','SIGTERM','SIGHUP'] as const)process.on(signal,quit);
