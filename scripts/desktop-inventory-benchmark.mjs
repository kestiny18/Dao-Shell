// Isolated real-WebView benchmark. No cloud model, source-file operations or global debug settings.
// node scripts/desktop-inventory-benchmark.mjs <fresh directory under .tools> <desktop exe>
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import net from 'node:net';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {performance} from 'node:perf_hooks';
const root=path.resolve(process.argv[2]||'');
const executable=path.resolve(process.argv[3]||'');
const toolsRoot=path.resolve('.tools')+path.sep;
const diagnostic=process.argv[4]==='--diagnose';
const resumeBudget=process.argv[4]==='--resume-budget';
const existing=diagnostic||resumeBudget;
if(!root.startsWith(toolsRoot)||root===toolsRoot||(!existing&&fs.existsSync(root))||!fs.existsSync(executable)) throw Error('Provide a NEW .tools fixture directory and an existing isolated desktop executable');
if(!existing) fs.mkdirSync(root,{recursive:true});
const files=path.join(root,'files-20000');if(!existing) fs.mkdirSync(files);
if(!existing) for(let folder=0;folder<20;folder++) {const dir=path.join(files,`group-${folder}`);fs.mkdirSync(dir);for(let i=0;i<1000;i++) fs.writeFileSync(path.join(dir,folder===0&&i===0?'rare-needle.txt':`common-doc-${folder}-${String(i).padStart(4,'0')}.txt`),'Synthetic benchmark metadata fixture.');}
const config=path.join(root,'config.json');
if(!existing) fs.writeFileSync(config,JSON.stringify({read_roots:[files],write_roots:[],access_mode:'full',model:null},null,2));
let evidence={created_at:new Date().toISOString(),fixture:root,files:20000,environment:{platform:os.platform(),release:os.release(),arch:os.arch(),cpu:os.cpus()[0]?.model,logical_cpus:os.cpus().length,node:process.version,build:'debug',cache:'warm OS cache; not a cold-cache claim'},samples:[],notes:['Desktop Rust process resource counters exclude WebView2 subprocess working sets.','UI time includes real Tauri perform, DOM rendering and two requestAnimationFrame callbacks in the test WebView. It is a paint opportunity upper bound, not physical display instrumentation.']};
if(resumeBudget){evidence=JSON.parse(fs.readFileSync(path.join(root,'evidence.json'),'utf8'));const settings=JSON.parse(fs.readFileSync(config,'utf8'));if(evidence.fixture!==root||evidence.samples.length!==90||settings.model!==null||settings.read_roots.length!==1||settings.read_roots[0]!==files)throw Error('Resume requires the unchanged synthetic fixture and its 90 samples');evidence.resumed_at=new Date().toISOString();}
evidence.binary_sha256=createHash('sha256').update(fs.readFileSync(executable)).digest('hex');
let client=null,pid=null;
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function freePort(){const server=net.createServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;await new Promise(r=>server.close(r));return port;}
class CDP {
 constructor(socket){this.socket=socket;this.next=0;this.pending=new Map();socket.addEventListener('message',e=>{const message=JSON.parse(e.data);if(message.id){const p=this.pending.get(message.id);if(!p)return;this.pending.delete(message.id);clearTimeout(p.timer);message.error?p.reject(Error(JSON.stringify(message.error))):p.resolve(message.result);}});}
 send(method,params={}){const id=++this.next;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.pending.delete(id);reject(Error(`CDP timeout: ${method}`));},180000);this.pending.set(id,{resolve,reject,timer});this.socket.send(JSON.stringify({id,method,params}));});}
 async evaluate(expression){const r=await this.send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;}
 close(){this.socket.close();for(const p of this.pending.values()){clearTimeout(p.timer);p.reject(Error('CDP closed'));}this.pending.clear();}
}
async function launch(){
 const port=await freePort(),started=performance.now();
 const env={...process.env,DAO_SHELL_DESKTOP_CONFIG:config,WEBVIEW2_USER_DATA_FOLDER:path.join(root,'webview-profile'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port} --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows`,P9_TEST_EXE:executable};
 pid=Number(execFileSync('powershell.exe',['-NoProfile','-Command','(Start-Process -FilePath $env:P9_TEST_EXE -WindowStyle Hidden -PassThru).Id'],{env,encoding:'utf8'}).trim());
 let page;
 for(let n=0;n<240;n++){try {page=(await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find(p=>p.type==='page'&&p.webSocketDebuggerUrl);if(page)break;}catch{}await sleep(250);}
 if(!page)throw Error('Test WebView did not expose its isolated debug endpoint');
 const socket=new WebSocket(page.webSocketDebuggerUrl);await new Promise((resolve,reject)=>{socket.addEventListener('open',resolve,{once:true});socket.addEventListener('error',reject,{once:true});});client=new CDP(socket);
 await client.send('Runtime.enable');await client.send('Performance.enable');await client.send('Page.bringToFront');
 for(let n=0;n<240;n++){if(await client.evaluate('typeof ready!=="undefined" && ready && !busy && Boolean(api)'))break;await sleep(250);if(n===239)throw Error('Desktop failed to become ready');}
 evidence.webview=await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
 await client.evaluate(`document.title='Dao-Shell P9 isolated benchmark';`);
 return performance.now()-started;
}
const invoke=(action,id=null,p=null)=>client.evaluate(`api.invoke('inventory_command',${JSON.stringify({action,id,path:p})})`);
async function waitStatus(id,predicate,timeout=150000){const start=performance.now();while(performance.now()-start<timeout){const rows=await invoke('status');const row=rows.find(r=>r.id===id);if(row&&predicate(row))return row;await sleep(100);}throw Error(`Inventory status timeout ${id}`);}
async function stableStatus(id){const started=performance.now();let signature=null,since=null;const transitions=[];while(performance.now()-started<30000){const row=(await invoke('status')).find(r=>r.id===id);const next=JSON.stringify([row.state,row.updated_at,row.full_scans,row.incremental_batches]);if(next!==signature){transitions.push({elapsed_ms:performance.now()-started,state:row.state,full_scans:row.full_scans,incremental_batches:row.incremental_batches,message:row.message});signature=next;since=performance.now();}if(row.state==='ready'&&performance.now()-since>=2000)return {row,transitions,settle_ms:performance.now()-started};await sleep(100);}throw Error('No stable ready snapshot within 30 seconds; possible update loop');}
async function query(kind,text){return client.evaluate(`(async()=>{const before=await api.invoke('inventory_command',{action:'status',id:null,path:null});const started=performance.now();const reply=await run('say',${JSON.stringify(text)});await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const elapsed=performance.now()-started;const r=reply?.search_receipt;if(!r)return {failure:'No actual search receipt in completed reply',ready,busy,mutatingWorkspace,active:workspace.active,before,reply,messages:session()?.messages.slice(-3),settings:await api.invoke('session_info')};return {kind:${JSON.stringify(kind)},ui_ms:elapsed,backend_ms:r.elapsed_ms,discovery_ms:r.discovery_ms,validation_ms:r.validation_ms,source:r.source,roots:r.roots,before,coverage_note:r.coverage_note,after:await api.invoke('inventory_command',{action:'status',id:null,path:null}),count:r.items.length,matched:r.matched_in_scan,skipped:r.skipped,truncated:r.truncated,visible_count:document.querySelectorAll('#candidate-list .file-row').length,page:currentPage,visibility:document.visibilityState};})()`);}
function counters(){return JSON.parse(execFileSync('powershell.exe',['-NoProfile','-Command','$p=Get-Process -Id $env:P9_TEST_PID; @{cpu_seconds=$p.CPU;working_set=$p.WorkingSet64;peak_working_set=$p.PeakWorkingSet64;private_bytes=$p.PrivateMemorySize64}|ConvertTo-Json -Compress'],{env:{...process.env,P9_TEST_PID:String(pid)},encoding:'utf8'}));}
async function close(){if(client){try{await client.evaluate("api.invoke('finish_close')");}catch{}client.close();client=null;}await sleep(1000);pid=null;}
function save(){fs.writeFileSync(path.join(root,diagnostic?'diagnostic.json':'evidence.json'),JSON.stringify(evidence,null,2));}
function stats(values){const sorted=[...values].sort((a,b)=>a-b);return {p50:sorted[Math.ceil(sorted.length*.5)-1],p95:sorted[Math.ceil(sorted.length*.95)-1],max:sorted.at(-1)};}
try {
 if(diagnostic){await launch();evidence.diagnostic=[];for(let i=0;i<6;i++){const before=await invoke('status');const sample=await query('diagnostic','common');const after=await invoke('status');evidence.diagnostic.push({before,sample,after});await sleep(500);}save();await close();console.log(JSON.stringify(evidence.diagnostic,null,2));process.exit(0);}
 let id=evidence.initial_snapshot?.id;
 if(!resumeBudget){
 console.log('Prepared 20,000 synthetic files; launching isolated desktop');evidence.initial_launch_ms=await launch();
 evidence.before_build=counters();const buildStart=performance.now();const rows=await invoke('add',null,files);id=rows.find(r=>r.root.replace(/^\\\\\?\\/,'').toLowerCase()===files.toLowerCase())?.id||rows[0]?.id;if(!id)throw Error('No registration');
 evidence.initial_snapshot=await waitStatus(id,r=>r.state==='ready');evidence.first_ready_ms=performance.now()-buildStart;evidence.bootstrap_settle=await stableStatus(id);evidence.initial_build_ms=performance.now()-buildStart;evidence.after_build=counters();console.log(`Initial build ${evidence.initial_build_ms.toFixed(1)}ms`);save();
 evidence.query_resources_before=counters();const queriesStart=performance.now();
 for(const [kind,text] of [['common','common'],['rare','rare-needle'],['absent','not-found-at-all']]){for(let n=0;n<30;n++){const sample=await query(kind,text);evidence.last_attempt=sample;if(sample.source!=='inventory'||sample.visibility!=='visible'||sample.page!=='files'||sample.count!==sample.visible_count)throw Error(`Invalid measured sample ${JSON.stringify(sample)}`);evidence.samples.push(sample);}console.log(`${kind}: ${JSON.stringify(stats(evidence.samples.filter(s=>s.kind===kind).map(s=>s.ui_ms)))}`);save();}
 evidence.query_duration_ms=performance.now()-queriesStart;evidence.query_resources_after=counters();evidence.renderer_metrics=await client.send('Performance.getMetrics');
 const updateStart=performance.now();await invoke('update',id);evidence.update_interactions=[];
 for(let n=0;n<5;n++){const status=(await invoke('status')).find(r=>r.id===id);const navigation=await client.evaluate(`(async()=>{const id=workspace.active,t=performance.now();selectTab('home');await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));selectTab(id);await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return performance.now()-t;})()`);evidence.update_interactions.push({state:status.state,navigation_ms:navigation});}
 evidence.query_during_update=await query('during_update','rare-needle');await waitStatus(id,r=>r.state==='ready');evidence.manual_update_ms=performance.now()-updateStart;save();
 await close();fs.writeFileSync(path.join(files,'offline-added.txt'),'Offline synthetic change');const restartStart=performance.now();evidence.restart_launch_ms=await launch();evidence.restart_snapshot=await waitStatus(id,r=>r.state==='ready');evidence.restart_to_ready_ms=performance.now()-restartStart;const offline=await query('offline','offline-added');if(offline.count!==1||offline.source!=='inventory')throw Error('Restart did not reconcile offline file');evidence.offline=offline;save();
 }else{console.log('Resuming budget stage; preserving the 90 completed measurements');await launch();}
 console.log('Preparing over-budget synthetic tree');const budget=path.join(root,'budget-100010');if(!fs.existsSync(budget)){fs.mkdirSync(budget);for(let f=0;f<10;f++){const dir=path.join(budget,`part-${f}`);fs.mkdirSync(dir);for(let i=0;i<10001;i++)fs.writeFileSync(path.join(dir,`budget-${i}.txt`),'');}}else{for(let f=0;f<10;f++)if(fs.readdirSync(path.join(budget,`part-${f}`)).length!==10001)throw Error('Incomplete budget fixture; preserve it for diagnosis');}
 let budgetRows=await invoke('status');if(!budgetRows.some(r=>r.id!==id))budgetRows=await invoke('add',null,budget);let budgetId=budgetRows.find(r=>r.id!==id).id;
 // Stop the initial update, prove stopped state persists, then explicitly retry to reach its real budget.
 await invoke('stop',budgetId);await sleep(600);evidence.stopped_budget=(await invoke('status')).find(r=>r.id===budgetId);if(evidence.stopped_budget.state!=='pending')throw Error('Stopped task restarted');save();
 const budgetStart=performance.now();await invoke('update',budgetId);evidence.budget_failure=await waitStatus(budgetId,r=>r.state==='pending'&&r.message.includes('预算'));evidence.budget_ms=performance.now()-budgetStart;save();
 await sleep(1200);evidence.budget_after_wait=(await invoke('status')).find(r=>r.id===budgetId);if(evidence.budget_after_wait.full_scans!==evidence.budget_failure.full_scans)throw Error('Budget task automatically retried');
 await invoke('remove',budgetId);budgetRows=await invoke('add',null,path.join(budget,'part-0'));const smallerId=budgetRows.find(r=>r.id!==id).id;const smallStart=performance.now();evidence.smaller_snapshot=await waitStatus(smallerId,r=>r.state==='ready');evidence.smaller_build_ms=performance.now()-smallStart;
 evidence.final_resources=counters();evidence.cache_bytes=fs.readdirSync(path.join(root,'file-inventory')).filter(f=>f.endsWith('.sqlite')).reduce((sum,f)=>sum+fs.statSync(path.join(root,'file-inventory',f)).size,0);
 evidence.summary=Object.fromEntries(['common','rare','absent'].map(kind=>[kind,Object.fromEntries(['ui_ms','backend_ms','discovery_ms','validation_ms'].map(metric=>[metric,stats(evidence.samples.filter(s=>s.kind===kind).map(s=>s[metric]))]))]));
 evidence.target_met=Object.values(evidence.summary).every(row=>row.ui_ms.p95<1000);evidence.completed_at=new Date().toISOString();save();console.log(JSON.stringify({summary:evidence.summary,target_met:evidence.target_met,evidence:path.join(root,'evidence.json')},null,2));
 await close();if(!evidence.target_met)process.exitCode=2;
} catch(error){evidence.failure=String(error.stack||error);save();console.error(error);await close();process.exitCode=1;}
