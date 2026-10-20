// Isolated Windows WebView self-test; fixed local model, synthetic files, no credentials.
// node scripts/computer-query-smoke.mjs <NEW .tools directory> <desktop exe>
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import net from 'node:net';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
const root=path.resolve(process.argv[2]||''),exe=path.resolve(process.argv[3]||'');
if(!root.startsWith(path.resolve('.tools')+path.sep)||fs.existsSync(root)||!fs.existsSync(exe)) throw Error('A NEW .tools directory and existing desktop executable are required');
fs.mkdirSync(root,{recursive:true}); const files=path.join(root,'files'); fs.mkdirSync(files);fs.writeFileSync(path.join(files,'synthetic-query.txt'),'Inert test fixture.');
const evidence={checks:[],errors:[],started_at:new Date().toISOString()};
let requests=0,client=null,pid=null;
const server=http.createServer((req,res)=>{let text='';req.on('data',chunk=>{text+=chunk;if(text.length>200000)req.destroy();});req.on('end',()=>{try{
  requests++;const body=JSON.parse(text),messages=body.messages,last=messages.at(-1),input=messages.findLast(m=>m.role==='user')?.content||'';
  const tool=(name,args)=>({role:'assistant',content:null,tool_calls:[{id:`test-${requests}`,type:'function',function:{name,arguments:JSON.stringify(args)}}]});
  let answer;
  if(last.role==='tool'){
    const value=JSON.parse(last.content);
    // Runtime projection must not send process identities, native paths or actions.
    if(value.kind){const t=JSON.stringify(value);assert(!t.includes('created_filetime'));assert(!t.includes('termination_ref'));assert(!t.includes('action_id'));}
    if(input==='fail-after-tool'){res.writeHead(503).end();return;}
    answer={role:'assistant',content:value.error?`Local policy: ${value.error}`:`Fixed local model: observed at ${value.observed_at}. This is a short scoped sample.`};
  }else if(input==='file'){answer=tool('file_search',{query:'synthetic'});}
  else if(input==='followup'){
    const previous=messages.filter(m=>m.role==='tool').map(m=>{try{return JSON.parse(m.content);}catch{return {};}}).findLast(v=>v.kind==='resource_snapshot');
    answer=tool('resource_snapshot',{observation_id:previous.observation_id,sections:['cpu'],limit:2});
  }else answer=tool('resource_snapshot',{sections:['cpu','memory'],sample_ms:input==='slow'?5000:500,limit:2});
  res.setHeader('Content-Type','application/json');res.end(JSON.stringify({choices:[{message:answer}]}));
}catch(error){evidence.errors.push(String(error));res.writeHead(400).end();}});});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const config=path.join(root,'config.json');fs.writeFileSync(config,JSON.stringify({read_roots:[files],computer_query_sharing:'ask',ui_language:'en',model:{endpoint:`http://127.0.0.1:${server.address().port}/chat/completions`,model:'Fixed local fixture',api_key_env:''}}));
const delay=ms=>new Promise(r=>setTimeout(r,ms));
class CDP{
 constructor(socket){this.socket=socket;this.id=0;this.pending=new Map();socket.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.id){const p=this.pending.get(m.id);if(p){this.pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}}});}
 send(method,params={}){const id=++this.id;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error(`Timed out: ${method}`)),30000);this.pending.set(id,{resolve,reject,timer});this.socket.send(JSON.stringify({id,method,params}));});}
 async evaluate(expression){const r=await this.send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;}
}
async function launch(){
 const probe=net.createServer();await new Promise(r=>probe.listen(0,'127.0.0.1',r));const port=probe.address().port;await new Promise(r=>probe.close(r));
 pid=Number(execFileSync('powershell.exe',['-NoProfile','-Command','(Start-Process -FilePath $env:DAO_QUERY_TEST_EXE -WindowStyle Hidden -PassThru).Id'],{encoding:'utf8',env:{...process.env,DAO_QUERY_TEST_EXE:exe,DAO_SHELL_DESKTOP_CONFIG:config,WEBVIEW2_USER_DATA_FOLDER:path.join(root,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`}}).trim());
 let page;for(let i=0;i<120;i++){try{page=(await(await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find(p=>p.type==='page'&&p.webSocketDebuggerUrl);if(page)break;}catch{}await delay(250);}
 if(!page)throw Error('No isolated WebView debugger');const socket=new WebSocket(page.webSocketDebuggerUrl);await new Promise((resolve,reject)=>{socket.addEventListener('open',resolve,{once:true});socket.addEventListener('error',reject,{once:true});});client=new CDP(socket);
 for(let i=0;i<120;i++){if(await client.evaluate("typeof ready !== 'undefined' && ready && !busy"))return;await delay(100);}throw Error('UI did not become ready');
}
async function check(label,expression){assert.equal(await client.evaluate(expression),true,label);evidence.checks.push(label);console.log(label);}
try{
 await launch();
 await check('ask refuses facts and displays local permission card',`(async()=>{const r=await run('say','resources');return !r.error && r.observations[0].error==='permission_required' && state(session().id).observations.some(v=>v.error==='permission_required');})()`);
 await client.evaluate("saveComputerSharing('enabled')");
 await check('enabled tool produces one deduplicated card',`(async()=>{const r=await run('say','resources');globalThis.firstObservation=r.observations[0];return !r.error && state(session().id).observations.filter(v=>v.observation_id===firstObservation.observation_id).length===1 && document.querySelectorAll('.computer-observation').length===1;})()`);
 await check('followup retains original observation and timestamp',`(async()=>{const r=await run('say','followup');return !r.error && r.observations[0].observation_id===firstObservation.observation_id && r.observations[0].observed_at===firstObservation.observed_at;})()`);
 await check('file query preserves computer observations',`(async()=>{const r=await run('say','file');return !r.error && r.items.length===1 && state(session().id).observations.length===1;})()`);
 await check('local process pagination preserves file candidates',`(async()=>{const r=await run('computer',JSON.stringify({tool:'process_list',arguments:{limit:2,sort:'name',sample_ms:500}}));const v=r.observations[0];const n=await run('computer',JSON.stringify({tool:'process_list',arguments:{limit:2,sort:'name',cursor:v.page.next_cursor}}));return !n.error && n.items.length===1 && n.observations[0].observation_id===v.observation_id && n.observations[0].data.processes[0].row_ref!==v.data.processes[0].row_ref;})()`);
 await check('new sample changes observation identity',`(async()=>{const r=await run('say','resources');return !r.error && r.observations[0].observation_id!==firstObservation.observation_id;})()`);
 await check('model failure preserves tool card and does not search',`(async()=>{const before=state(session().id).observations.length;const r=await run('say','fail-after-tool');return r.error && r.observations.length===1 && r.search_receipt===null && state(session().id).observations.length>=before;})()`);
 await check('cancellation prevents unfinished sample publication',`(async()=>{const task=run('say','slow');await new Promise(r=>setTimeout(r,200));await stopRequest();const r=await task;return r.error && r.observations.every(v=>!v.observation_id);})()`);
 await check('disabled sharing invalidates contexts and references',`(async()=>{await saveComputerSharing('disabled');return workspace.sessions.every(s=>state(s.id).observations.length===0) && (await run('say','resources')).observations[0].error==='permission_denied';})()`);
 const before=requests;
 await check('local queries remain available with sharing disabled',`(async()=>{for(const tool of computerTools){const args=['resource_snapshot','process_list','network_snapshot'].includes(tool)?{sample_ms:500}:{};const r=await run('computer',JSON.stringify({tool,arguments:args}));if(r.error || r.observations.length!==1)return false;}return true;})()`);assert.equal(requests,before);
 await check('cards are scoped to originating session',`(async()=>{const owner=session().id,old=state(owner).observations.length;await newSession();if(document.querySelectorAll('.computer-observation').length)return false;selectTab(owner);return document.querySelectorAll('.computer-observation').length===old;})()`);
 await check('no missing localized message keys',"missingMessageKeys.size===0");
 // Keep detailed raw observations out of workspace persistence.
 await check('workspace stores receipts, not live observations',`(async()=>{await persist();return workspace.sessions.every(s=>s.messages.every(m=>!('observations' in m)&&!('data' in m)));})()`);
 await client.evaluate("$('computer-observations').scrollIntoView({block:'start'});true");
 const image=await client.send('Page.captureScreenshot',{format:'png'});fs.writeFileSync(path.join(root,'computer-cards.png'),Buffer.from(image.data,'base64'));
 await client.evaluate("setTimeout(()=>api.invoke('finish_close'),100);true");client.socket.close();client=null;pid=null;
 await delay(1200); await launch();
 await check('restart restores text history without live cards or references',"workspace.sessions.some(s=>s.messages.some(m=>m.role==='receipt' && m.text.includes('Historical observation'))) && workspace.sessions.every(s=>state(s.id).observations.length===0) && document.querySelectorAll('.computer-observation').length===0");
 await check('no-model mode preserves explicit local query entry',`(async()=>{const view=await api.invoke('get_settings');view.config.models.active=null;await api.invoke('save_settings',{update:{expected_revision:view.revision,config:view.config,keys:[]}});invalidateContexts();await init();const r=await run('say','resources');const local=await run('computer',JSON.stringify({tool:'resource_snapshot',arguments:{sections:['disks']}}));return r.error && r.search_receipt===null && !local.error;})()`);
 await check('Chinese and English labels remain complete',"applyLanguage('zh-CN'); missingMessageKeys.size===0");
 await client.evaluate("$('computer-observations').scrollIntoView({block:'start'});true");
 const zh=await client.send('Page.captureScreenshot',{format:'png'});fs.writeFileSync(path.join(root,'computer-cards-zh.png'),Buffer.from(zh.data,'base64'));
 await client.evaluate("setTimeout(()=>api.invoke('finish_close'),100);true");client.socket.close();client=null;pid=null;
 evidence.result='PASS';
}catch(error){evidence.result='FAIL';evidence.errors.push(String(error));process.exitCode=1;console.error(error);}
finally{if(client){try{await client.evaluate("setTimeout(()=>api.invoke('finish_close'),100);true");}catch{}client.socket.close();}server.close();fs.writeFileSync(path.join(root,'evidence.json'),JSON.stringify({...evidence,model_requests:requests,completed_at:new Date().toISOString()},null,2));}
