// Isolated Debug WebView trial. All native writes are rejected; no DNS flush or app install.
import fs from 'node:fs';import path from 'node:path';import net from 'node:net';import assert from 'node:assert/strict';import {execFileSync} from 'node:child_process';
const root=path.resolve('.tools',`maintenance-smoke-${Date.now()}`);fs.mkdirSync(root,{recursive:true});
const exe=path.resolve('desktop/src-tauri/target/debug/dao-shell-desktop.exe');
const config=path.join(root,'config.json');fs.writeFileSync(config,JSON.stringify({read_roots:[root],computer_query_sharing:'disabled',ui_language:'zh-CN'}));
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const evidence={root,checks:[],native_writes:'NOT_EXECUTED',real_package_provider:'NOT_VERIFIED'};let socket,client,identity;
class CDP{
 constructor(socket){this.socket=socket;this.id=0;this.pending=new Map();socket.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.id){const p=this.pending.get(m.id);if(p){this.pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}}});}
 send(method,params={}){const id=++this.id;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error(`Timed out: ${method}`)),30000);this.pending.set(id,{resolve,reject,timer});this.socket.send(JSON.stringify({id,method,params}));});}
 async evaluate(expression){const r=await this.send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;}
}
async function check(label,expression){assert.equal(await client.evaluate(expression),true,label);evidence.checks.push(label);console.log(label);}
try{
 const server=net.createServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;await new Promise(r=>server.close(r));
 execFileSync('powershell.exe',['-NoProfile','-Command',"$owned=Start-Process -FilePath $env:DAO_TEST_DESKTOP -WindowStyle Hidden -RedirectStandardOutput $env:DAO_TEST_STDOUT -RedirectStandardError $env:DAO_TEST_STDERR -PassThru; @{pid=$owned.Id;start_ticks=$owned.StartTime.ToUniversalTime().Ticks.ToString()} | ConvertTo-Json -Compress | Set-Content -LiteralPath $env:DAO_TEST_IDENTITY -Encoding UTF8"],{stdio:'ignore',timeout:15000,encoding:'utf8',env:{...process.env,DAO_TEST_IDENTITY:path.join(root,'process.json'),DAO_TEST_DESKTOP:exe,DAO_TEST_STDOUT:path.join(root,'startup.out'),DAO_TEST_STDERR:path.join(root,'startup.err'),DAO_SHELL_DENY_NATIVE_DNS:'1',DAO_SHELL_DESKTOP_CONFIG:config,WEBVIEW2_USER_DATA_FOLDER:path.join(root,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`}});
 identity=JSON.parse(fs.readFileSync(path.join(root,'process.json'),'utf8').replace(/^\uFEFF/,''));
 evidence.process=identity;evidence.startup_budget_ms=60000;
 let page;for(let deadline=Date.now()+60000;Date.now()<deadline;){try{page=(await(await fetch(`http://127.0.0.1:${port}/json/list`,{signal:AbortSignal.timeout(1000)})).json()).find(p=>p.type==='page'&&p.webSocketDebuggerUrl);if(page)break;}catch{}await delay(250);}assert(page,'No isolated WebView');
 socket=new WebSocket(page.webSocketDebuggerUrl);await new Promise((r,j)=>{socket.addEventListener('open',r,{once:true});socket.addEventListener('error',j,{once:true});});client=new CDP(socket);
 for(let i=0;i<120;i++){if(await client.evaluate("typeof ready!=='undefined'&&ready&&!busy"))break;await delay(100);}
 await client.evaluate("run('maintenance-local',JSON.stringify({kind:'debug_fixture_status'}))");
 await check('Debug native-DNS deny latch is active before testing confirmations',"state(workspace.active).maintenanceResult.native_dns_dispatch_denied===true");
 await client.evaluate("run('maintenance-local',JSON.stringify({kind:'history'}))");
 await check('Local maintenance is available without model or fact sharing',"Boolean(session()&&document.getElementById('maintenance-panel')) && state(workspace.active).maintenanceResult.receipts.length===0");
 await client.evaluate("globalThis.trial=run('maintenance',JSON.stringify({tool:'network_maintenance',arguments:{kind:'flush_dns_cache',confirmed:true}}));true");
 await client.evaluate('trial');await check('Forged approval does not open a confirmation',"!state(workspace.active).maintenanceConfirmation && !busy");
 await client.evaluate("globalThis.trial=run('maintenance',JSON.stringify({tool:'network_maintenance',arguments:{kind:'flush_dns_cache'}}));true");
 for(let i=0;i<120;i++){if(await client.evaluate('Boolean(state(workspace.active).maintenanceConfirmation)'))break;await delay(100);}
 await check('Fixed DNS action shows local risk before dispatch',"Boolean(state(workspace.active).maintenanceConfirmation) && document.getElementById('maintenance-panel').textContent.includes('无法恢复原缓存')");
 await check('Generic approval IPC cannot approve maintenance',"(async()=>{const c=state(workspace.active).maintenanceConfirmation;try{await api.invoke('confirm_open',{sessionId:workspace.active,executionId:c.executionId,requestId:c.request_id,approved:true});return false;}catch{return true;}})()");
 await client.evaluate('answerMaintenance(false)');await client.evaluate('trial');
 await check('Refusal reports cancelled before dispatch',"state(workspace.active).maintenanceResult.status==='cancelled_before_dispatch' && !busy");
 await client.evaluate("run('maintenance-local',JSON.stringify({kind:'history'}))");
 await check('Refusal leaves no dispatch receipt',"state(workspace.active).maintenanceResult.receipts.length===0");
 await check('Confirmation token and native plan stay out of saved history',"!JSON.stringify(workspace).includes('request_id') && !JSON.stringify(workspace).includes('agreement_digest')");
 evidence.status='PASS';
}catch(error){evidence.status='FAIL';evidence.error=String(error);process.exitCode=1;}
finally{
 if(client){try{await client.evaluate("setTimeout(()=>api.invoke('finish_close'),100);true");await delay(500);}catch{}socket?.close();}
 if(identity){try{execFileSync('powershell.exe',['-NoProfile','-Command',"$owned=Get-Process -Id $env:DAO_TEST_PID -ErrorAction SilentlyContinue; if($owned -and $owned.StartTime.ToUniversalTime().Ticks.ToString() -eq $env:DAO_TEST_START -and $owned.Path -eq $env:DAO_TEST_DESKTOP){$owned | Stop-Process -ErrorAction SilentlyContinue}"],{env:{...process.env,DAO_TEST_PID:String(identity.pid),DAO_TEST_START:identity.start_ticks,DAO_TEST_DESKTOP:exe}});}catch{}}
 fs.writeFileSync(path.join(root,'evidence.json'),JSON.stringify(evidence,null,2));console.log(JSON.stringify(evidence,null,2));
}
