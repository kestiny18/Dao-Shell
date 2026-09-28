import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import { Script } from 'node:vm';

const clone = (value) => JSON.parse(JSON.stringify(value));
const file = { id:'internal-object-id', path:'C:\\fixture\\合同 <script>.txt', identity:{ directory:false, size:12 } };
async function fixture(options = {}) {
  const dom = new JSDOM(await readFile(new URL('../ui/index.html', import.meta.url), 'utf8'), { runScripts:'outside-only', url:'http://tauri.localhost/' });
  const w = dom.window; w.TextEncoder = TextEncoder; w.HTMLElement.prototype.scrollIntoView = function () {};
  const calls = []; let current = { revision:'original', credential_available:[], config:{ read_roots:['C:\\fixture'], write_roots:[], access_mode:'restricted', appearance:'light', model:null, models:{ connections:[{id:'local',label:'Local',endpoint:'http://127.0.0.1:12345/v1/chat/completions',api_key_env:''}], choices:[{id:'fixture',connection_id:'local',name:'fixture-model'}],active:'fixture' } } };
  let pending = null;
  let savedWorkspace = options.workspace || {version:1,sessions:[],tabs:['home'],active:'home',sidebar_width:205,collapsed:false,home_draft:''};
  const events = []; const windowEvents = new Map();
  let saveFailure = false; let saveGate = null;

  w.__TAURI__ = { event:{listen:async(name,handler)=>{windowEvents.set(name,handler);}}, core:{ Channel:class {}, invoke:async (command, args) => {
    calls.push({command,args:clone(args || {})});
    if (command === 'load_workspace') { if (options.loadFailure) throw Error('broken workspace'); return clone(savedWorkspace); }
    if (command === 'save_workspace') { if (saveGate) await saveGate; if (saveFailure) throw Error('disk full'); savedWorkspace = clone(args.workspace); return; }
    if (['register_session','finish_close'].includes(command)) return;
    if (command === 'session_info') return { roots:current.config.read_roots, model:'fixture-model', model_error:null, access_mode:current.config.access_mode };
    if (command === 'get_settings') return clone(current);
    if (command === 'save_settings') { assert.equal(args.update.expected_revision, current.revision); current = { revision:'saved', credential_available:[],config:clone(args.update.config) }; return clone(current); }
    if (command === 'test_connection') return { elapsed_ms:20,total_tokens:null };
    if (command === 'computer_overview') return {system:{cpu_percent:12.5,memory_used_bytes:1024,memory_total_bytes:2048,memory_available_bytes:1024,disks:[{mount:'C:\\',total_bytes:4096,available_bytes:2048}]},device:{host:'Fixture computer',os:'Fixture OS',uptime_seconds:3600,architecture:'x86_64'},logical_cpus:2,observed_at:'2026-09-21T10:00:00Z',network:{adapters:[],limits:'Fixture counters'},applications:{items:[{name:'<img src=x onerror=alert(1)>',version:'1'}],available:true,partial:true,limits:'Partial fixture'}};
    if (command === 'perform') {
      const emit = event => args.onEvent.onmessage({...event,session_id:args.sessionId,execution_id:args.requestId}); events.push(emit);
      if (args.input === 'slow') return new Promise((resolve) => { pending = resolve; });
      const items = args.input === 'absent' ? [] : [file];
      emit({kind:'result',capability:'file_search',value:{items}});
      return {message:'fixture response',error:false,items};
    }
    if (command === 'cancel') { if (!options.deferCancel) pending?.({message:'本轮已取消',error:true,items:[]}); return; }
    throw Error(`Unexpected command ${command}`);
  } } };
  for (const script of ['app.js','layout.js','settings.js','home.js']) new Script(await readFile(new URL(`../ui/${script}`, import.meta.url), 'utf8'), { filename:script }).runInContext(dom.getInternalVMContext());
  const tick = async () => { for(let i=0;i<5;i++) await new Promise((resolve) => setTimeout(resolve,0)); };
  await tick();
  const $ = (id) => w.document.getElementById(id);
  const change = (id,value) => { $(id).value=value; $(id).dispatchEvent(new w.Event('change',{bubbles:true})); };
  const submit = (id) => $(id).dispatchEvent(new w.Event('submit',{bubbles:true,cancelable:true}));
  return {dom,w,$,calls,tick,change,submit,events,windowEvents,saved:()=>clone(savedWorkspace),failSave:value=>{saveFailure=value;},holdSave:()=>{let release;saveGate=new Promise(resolve=>{release=resolve;});return ()=>{saveGate=null;release();};},finishRequest:()=>pending?.({message:'本轮已取消',error:true,items:[]})};
}

for (const failed of [false,true]) for (const callback of ['progress','result','finally']) {
  test(`close preserves draft ownership during ${callback} and ${failed ? 'failed' : 'successful'} delayed save`,async(t)=>{
    const f=await fixture();t.after(()=>f.dom.window.close());
    f.$('input').value='slow';f.submit('composer');await f.tick();const a=f.saved().active;
    f.$('input').value='draft A';f.$('input').dispatchEvent(new f.w.Event('input'));await f.tick();
    f.$('reset').click();await f.tick();const b=f.saved().active;
    f.$('input').value='draft B';f.$('input').dispatchEvent(new f.w.Event('input'));await f.tick();
    const release=f.holdSave();f.failSave(failed);
    f.w.document.querySelector('[aria-label="关闭标签：新会话"]').click();
    await f.tick();
    assert.equal(f.w.eval('workspace.active'),a);
    assert.equal(f.$('input').value,'draft A');
    assert.equal(f.$('page-title').textContent,'slow');
    if(callback==='finally') f.finishRequest();
    else f.events[0](callback==='progress' ? {kind:'progress',text:'still working'} : {kind:'result',capability:'file_search',value:{items:[file]}});
    await f.tick();release();await f.tick();
    const live=clone(f.w.eval('workspace'));
    assert.equal(live.sessions.find(s=>s.id===a).draft,'draft A');
    assert.equal(live.active,failed ? b : a);
    assert.equal(f.$('input').value,failed ? 'draft B' : 'draft A');
    assert.equal(live.sessions.find(s=>s.id===b)?.draft,'draft B');
    if(failed) {assert.match(f.$('workspace-status').textContent,/disk full/);f.failSave(false);f.$('retry-save').click();await f.tick();}
    assert.equal(f.saved().sessions.find(s=>s.id===a).draft,'draft A');
    assert.equal(f.saved().sessions.find(s=>s.id===b)?.draft,'draft B');
    const restored=await fixture({workspace:f.saved()});t.after(()=>restored.dom.window.close());
    assert.equal(restored.$('input').value,failed ? 'draft B' : 'draft A');
    f.finishRequest();await f.tick();
  });
}

test('home remains local, renders facts and treats application names as text', async (t) => {
  const f=await fixture();t.after(()=>f.dom.window.close());
  assert.equal(f.$('cpu-value').textContent,'12.5%');
  assert.equal(f.calls.filter((c)=>c.command==='perform').length,0);
  assert.equal(f.$('applications').querySelector('img'),null);
  assert.match(f.$('app-note').textContent,/部分注册表/);
  f.$('explain-home').click();await f.tick();
  assert.equal(f.calls.find((c)=>c.command==='perform').args.kind,'explain');
});

test('file names hide internal references, navigation preserves conversation, empty search clears candidates',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('input').value='contract';f.submit('composer');await f.tick();
  assert.equal(f.$('candidate-list').querySelector('.file-name').textContent,'合同 <script>.txt');
  assert.equal(f.$('candidate-list').querySelector('script'),null);
  assert.ok(!f.$('candidate-list').textContent.includes('internal-object-id'));
  const scroller=f.w.document.querySelector('.content-scroll');scroller.scrollTop=125;
  f.w.document.querySelector('[data-page="settings"]').click();
  assert.equal(scroller.scrollTop,0);
  f.w.document.querySelector('[data-page="files"]').click();
  assert.equal(scroller.scrollTop,125);
  assert.match(f.$('conversation').textContent,/fixture response/);
  f.$('input').value='absent';f.submit('composer');await f.tick();
  assert.equal(f.$('candidate-panel').hidden,true);
  f.$('input').value='slow';f.submit('composer');await f.tick();
  assert.equal(f.$('cancel').hidden,false);assert.equal(f.$('send').disabled,true);
  f.$('cancel').click();await f.tick();assert.equal(f.$('send').disabled,false);
});

test('settings retain multiple connections, test drafts, save selected model and clear entered keys',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.w.document.querySelector('[data-page="settings"]').click();
  f.$('add-connection').click();
  f.change('connection-label','Synthetic service');
  f.change('connection-endpoint','https://example.invalid/v1/chat/completions');
  f.change('connection-models','model-a\nmodel-b');
  f.change('connection-key','synthetic-secret');f.$('persist-key').checked=false;
  const options=[...f.$('active-model').options];f.change('active-model',options.find((o)=>o.textContent.endsWith('model-b')).value);
  f.$('test-model').click();await f.tick();
  assert.match(f.$('settings-status').textContent,/尚未保存/);
  f.change('appearance','dark');f.submit('settings-form');await f.tick();
  const saved=f.calls.find((c)=>c.command==='save_settings').args.update;
  assert.equal(saved.config.models.connections.length,2);
  assert.equal(saved.config.models.choices.find((m)=>m.id===saved.config.models.active).name,'model-b');
  assert.equal(saved.keys[0].key,'synthetic-secret');
  assert.ok(!JSON.stringify(saved.config).includes('synthetic-secret'));
  assert.equal(f.$('connection-key').value,'');
  assert.equal(f.w.document.documentElement.dataset.theme,'dark');
});

test('provider choices are complete and switching services updates the active model without forwarding a key',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  assert.deepEqual([...f.$('provider-preset').options].map(o=>o.textContent),['DeepSeek','OpenAI','自定义']);
  f.change('provider-preset','deepseek');
  assert.equal(f.$('connection-endpoint').value,'https://api.deepseek.com/chat/completions');
  assert.equal(f.$('endpoint-field').hidden,true);
  assert.equal(f.$('custom-model-field').hidden,true);
  f.change('connection-key','synthetic-deepseek-key');
  f.change('provider-preset','openai');
  assert.equal(f.$('connection-key').value,'');
  assert.equal(f.$('connection-endpoint').value,'https://api.openai.com/v1/chat/completions');
  assert.equal(f.$('model-preset').value,'gpt-5-mini');
  f.change('model-preset','gpt-4.1');f.submit('settings-form');await f.tick();
  const saved=f.calls.find(c=>c.command==='save_settings').args.update;
  assert.equal(saved.keys.length,0);
  assert.equal(saved.config.models.choices.find(m=>m.id===saved.config.models.active).name,'gpt-4.1');
  assert.equal(f.$('provider-preset').value,'openai');
  f.change('provider-preset','custom');assert.equal(f.$('endpoint-field').hidden,false);
  f.change('connection-endpoint','http://127.0.0.1:11434/v1/chat/completions');
  f.change('connection-models','local-model');f.submit('settings-form');await f.tick();
  assert.equal(f.$('provider-preset').value,'custom');
  assert.equal(f.$('connection-env').value,'');
  f.change('connection-endpoint','https://example.invalid/v1/chat/completions');
  f.change('connection-key','synthetic-custom-key');f.$('persist-key').checked=false;
  f.submit('settings-form');await f.tick();
  const custom=f.calls.filter(c=>c.command==='save_settings').at(-1).args.update;
  assert.match(custom.config.models.connections[0].api_key_env,/^DAO_MODEL_/);
  assert.equal(custom.keys[0].key,'synthetic-custom-key');
});

test('sidebar resizing is bounded and application view changes preserve filtering',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('sidebar-resize').dispatchEvent(new f.w.KeyboardEvent('keydown',{key:'End',bubbles:true}));
  await f.tick(); assert.equal(f.saved().sidebar_width,360);
  f.w.innerWidth=680;f.w.dispatchEvent(new f.w.Event('resize'));
  assert.equal(f.$('sidebar-resize').getAttribute('aria-valuenow'),'220');
  assert.equal(f.$('applications').dataset.view,'cards');
  f.$('app-filter').value='no match';f.$('app-filter').dispatchEvent(new f.w.Event('input'));
  f.w.document.querySelector('[data-app-view="list"]').click();
  assert.equal(f.$('applications').dataset.view,'list');
  assert.match(f.$('applications').textContent,/没有匹配/);
  assert.equal(f.w.localStorage.getItem('dao.appView'),'list');
});


test('workspace drafts, tab uniqueness, close and restart preserve only visible history',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('input').value='first request';f.submit('composer');await f.tick();
  const a=f.saved().sessions[0].id;
  f.$('input').value='draft A';f.$('input').dispatchEvent(new f.w.Event('input'));await f.tick();
  f.$('reset').click();await f.tick();const b=f.saved().active;
  f.$('input').value='draft B';f.$('input').dispatchEvent(new f.w.Event('input'));await f.tick();
  f.w.document.querySelector(`[aria-label="打开会话：first request"]`).click();await f.tick();
  assert.equal(f.$('input').value,'draft A');
  f.w.document.querySelector(`[aria-label="关闭标签：first request"]`).click();await f.tick();
  assert.equal(f.saved().sessions.length,2);assert.ok(!f.saved().tabs.includes(a));
  f.w.document.querySelector(`[aria-label="打开会话：first request"]`).click();await f.tick();
  f.w.document.querySelector(`[aria-label="打开会话：first request"]`).click();await f.tick();
  assert.equal(f.saved().tabs.filter(id=>id===a).length,1);
  assert.equal(f.saved().sessions.find(s=>s.id===b).draft,'draft B');
  assert.ok(!JSON.stringify(f.saved()).includes('internal-object-id'));
  const restored=await fixture({workspace:f.saved()});t.after(()=>restored.dom.window.close());
  assert.equal(restored.$('input').value,'draft A');assert.equal(restored.$('context-notice').hidden,false);
  assert.equal(restored.$('candidate-panel').hidden,true);
  assert.equal(restored.calls.filter(c=>c.command==='perform').length,0);
  assert.match(restored.$('conversation').textContent,/fixture response/);
  assert.match(restored.$('conversation').textContent,/合同 <script>.txt/);
  assert.equal(restored.$('conversation').querySelector('script'),null);
  assert.equal(restored.$('workspace-tabs').firstChild.textContent,'概览');
});

test('switch during request routes events to owner; stop-close waits and late events stay dead',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('input').value='slow';f.submit('composer');await f.tick(); const a=f.saved().active;
  f.$('reset').click();await f.tick();const b=f.saved().active;
  assert.notEqual(a,b);assert.equal(f.$('send').disabled,true);
  f.events[0]({kind:'result',capability:'file_search',value:{items:[file]}});
  assert.equal(f.$('candidate-panel').hidden,true);
  f.w.document.querySelector('[aria-label="打开会话：slow"]').click();await f.tick();
  assert.equal(f.$('candidate-panel').hidden,false);
  f.w.document.querySelector('[aria-label="关闭标签：slow"]').click();await f.tick();
  assert.equal(f.$('decision').hidden,false);f.$('decision-no').click();await f.tick();
  assert.equal(f.calls.filter(c=>c.command==='cancel').length,0);
  f.w.eval('closeTab(workspace.active)');await f.tick();f.$('decision-yes').click();await f.tick();
  assert.ok(f.saved().sessions.some(s=>s.id===a));assert.ok(!f.saved().tabs.includes(a));
  assert.equal(f.calls.find(c=>c.command==='cancel').args.sessionId,a);
  f.events[0]({kind:'confirm_open',request_id:'late',object:file});
  assert.equal(f.$('confirmation').hidden,true);assert.ok(f.saved().sessions.some(s=>s.id===a));assert.ok(!f.saved().tabs.includes(a));
});

test('confirmation never approves on another tab; setting save preserves history and invalidates candidates',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('input').value='slow';f.submit('composer');await f.tick();
  f.events[0]({kind:'confirm_open',request_id:'confirm',object:file});
  assert.equal(f.$('confirmation').hidden,false);
  f.$('reset').click();await f.tick();f.$('approve').click();await f.tick();
  assert.equal(f.calls.filter(c=>c.command==='confirm_open').length,0);
  f.$('cancel').click();await f.tick();
  f.w.document.querySelector('[aria-label="打开会话：slow"]').click();await f.tick();
  f.w.document.querySelector('[data-page="settings"]').click();await f.tick();f.submit('settings-form');await f.tick();
  f.w.document.querySelector('[aria-label="打开会话：slow"]').click();await f.tick();
  assert.match(f.$('conversation').textContent,/slow/);assert.equal(f.$('context-notice').hidden,false);
});

test('failed save retains closed tab and session; corrupt load blocks all writes; IME never submits',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('input').value='keep';f.submit('composer');await f.tick();f.failSave(true);
  const active=f.saved().active;
  f.w.eval('closeTab(workspace.active)');await f.tick();
  assert.equal(f.saved().sessions.length,1);assert.ok(f.saved().tabs.includes(active));
  assert.equal(f.w.eval('workspace.active'),active);
  assert.match(f.$('workspace-status').textContent,/disk full/);
  f.failSave(false);f.$('retry-save').click();await f.tick();assert.equal(f.$('workspace-status').hidden,true);
  const broken=await fixture({loadFailure:true});t.after(()=>broken.dom.window.close());
  broken.$('reset').click();await broken.tick();assert.equal(broken.calls.filter(c=>c.command==='save_workspace').length,0);
  assert.match(broken.$('workspace-status').textContent,/恢复失败/);
  const count=f.calls.filter(c=>c.command==='perform').length;
  f.$('input').value='中文';
  for(const event of [{key:'Enter',isComposing:true},{key:'Enter',keyCode:229},{key:'Enter',shiftKey:true}]) f.$('input').dispatchEvent(new f.w.KeyboardEvent('keydown',{...event,bubbles:true,cancelable:true}));
  await f.tick();assert.equal(f.calls.filter(c=>c.command==='perform').length,count);
});


test('window close flushes latest draft and asks before abandoning failed save',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('reset').click();await f.tick();
  f.$('input').value='latest unsent draft';
  await f.windowEvents.get('workspace-close-requested')();
  assert.equal(f.saved().sessions[0].draft,'latest unsent draft');
  assert.equal(f.calls.at(-1).command,'finish_close');
  f.failSave(true);f.$('input').value='must retain';
  const closing=f.windowEvents.get('workspace-close-requested')();await f.tick();
  assert.equal(f.$('decision').hidden,false);f.$('decision-no').click();await closing;
  assert.equal(f.calls.filter(c=>c.command==='finish_close').length,1);
  assert.equal(f.$('input').value,'must retain');
});

test('closing a running session waits for completion and ignores stale events during a later request',async(t)=>{
  const f=await fixture({deferCancel:true});t.after(()=>f.dom.window.close());
  f.$('input').value='slow';f.submit('composer');await f.tick();
  const a=f.saved().active;
  f.w.eval('closeTab(workspace.active)');await f.tick();
  f.$('decision-yes').click();await f.tick();
  assert.ok(f.saved().tabs.includes(a));
  assert.ok(f.saved().sessions.some(s=>s.id===a));
  assert.equal(f.$('send').disabled,true);
  f.events[0]({kind:'confirm_open',request_id:'after-stop',object:file});
  assert.equal(f.$('confirmation').hidden,true);
  f.finishRequest();await f.tick();
  assert.ok(f.saved().sessions.some(s=>s.id===a));assert.ok(!f.saved().tabs.includes(a));
  f.$('input').value='slow';f.submit('composer');await f.tick();
  f.events[0]({kind:'result',capability:'file_search',value:{items:[file]}});
  f.events[0]({kind:'confirm_open',request_id:'old',object:file});
  assert.equal(f.$('candidate-panel').hidden,true);
  assert.equal(f.$('confirmation').hidden,true);
  assert.ok(!f.$('conversation').textContent.includes(file.path));
  const current=f.calls.filter(c=>c.command==='perform').at(-1);
  // Inject mismatched ownership without the fixture's automatic identity tagging.
  const raw=f.w.eval('activeRequest');
  f.w.eval('onEvent')({kind:'confirm_open',session_id:a,execution_id:current.args.requestId,request_id:'foreign',object:file},raw);
  assert.equal(f.$('confirmation').hidden,true);
  f.finishRequest();await f.tick();
});

test('legacy collapsed workspace expands and settings singleton survives restart',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  assert.equal(f.$('collapse-sidebar'),null);
  for(let i=0;i<3;i++) f.w.document.querySelector('[data-page="settings"]').click();
  await f.tick();assert.equal(f.saved().tabs.filter(id=>id==='settings').length,1);
  f.w.document.querySelector('[aria-label="关闭标签：设置"]').click();await f.tick();
  assert.deepEqual(f.saved().tabs,['home']);assert.equal(f.saved().active,'home');
  const restored=await fixture({workspace:{...f.saved(),collapsed:true}});t.after(()=>restored.dom.window.close());
  assert.equal(restored.w.document.querySelector('.shell').classList.contains('sidebar-collapsed'),false);
  assert.equal(restored.w.eval('workspace.collapsed'),false);
  assert.equal(restored.$('collapse-sidebar'),null);
  assert.equal(restored.w.document.querySelector('[aria-label="关闭标签：概览"]'),null);
});

test('settings button retains a descriptive accessible label',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  assert.equal(f.$('collapse-sidebar'),null);
  const settings=f.w.document.querySelector('.bottom-nav [data-page="settings"]');
  // Keep the explicit accessible name when presentation changes.
  assert.match(settings.getAttribute('aria-label') || settings.getAttribute('title') || '',/设置/);
});


test('rename saves trimmed text to list and tab, restores it, and has no delete UI',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('reset').click();await f.tick();
  f.$('input').value='draft stays';f.$('input').dispatchEvent(new f.w.Event('input'));await f.tick();
  assert.equal(f.w.document.querySelector('[aria-label^="删除会话"]'),null);
  f.w.document.querySelector('.session-rename').click();
  let input=f.w.document.querySelector('.session-rename-form input');input.value='  我的项目  ';
  f.w.document.querySelector('[aria-label="保存名称"]').click();await f.tick();
  assert.equal(f.saved().sessions[0].title,'我的项目');
  assert.equal(f.saved().sessions[0].draft,'draft stays');
  assert.ok(f.w.document.querySelector('[aria-label="关闭标签：我的项目"]'));
  assert.equal(f.$('page-title').textContent,'我的项目');
  const restored=await fixture({workspace:f.saved()});t.after(()=>restored.dom.window.close());
  assert.ok(restored.w.document.querySelector('[aria-label="打开会话：我的项目"]'));
  restored.$('input').value='another request';restored.submit('composer');await restored.tick();
  assert.equal(restored.saved().sessions[0].title,'我的项目');
});

test('rename cancellation, byte limit and failed persistence preserve title and draft',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('reset').click();await f.tick();
  f.$('input').value='unsent';f.$('input').dispatchEvent(new f.w.Event('input'));await f.tick();
  f.w.document.querySelector('.session-rename').click();
  let input=f.w.document.querySelector('.session-rename-form input');input.value='discard';
  input.dispatchEvent(new f.w.KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
  assert.equal(f.w.document.querySelector('.session-rename-form'),null);
  assert.equal(f.saved().sessions[0].title,'新会话');assert.equal(f.$('input').value,'unsent');
  f.w.document.querySelector('.session-rename').click();input=f.w.document.querySelector('.session-rename-form input');
  for(const value of ['   ','汉'.repeat(171)]) {
    input.value=value;f.w.document.querySelector('[aria-label="保存名称"]').click();await f.tick();
    assert.ok(f.w.document.querySelector('.rename-error').textContent);assert.equal(f.saved().sessions[0].title,'新会话');
  }
  input.value='汉'.repeat(170);f.failSave(true);
  f.w.document.querySelector('[aria-label="保存名称"]').click();await f.tick();
  assert.match(f.w.document.querySelector('.rename-error').textContent,/名称未保存/);
  assert.equal(f.w.eval('workspace.sessions[0].title'),'新会话');assert.equal(f.$('input').value,'unsent');
  f.failSave(false);f.w.document.querySelector('[aria-label="保存名称"]').click();await f.tick();
  assert.equal(f.saved().sessions[0].title,'汉'.repeat(170));assert.equal(f.saved().sessions[0].draft,'unsent');
});

test('request callbacks preserve rename editor and custom title including placeholder name',async(t)=>{
  const f=await fixture();t.after(()=>f.dom.window.close());
  f.$('input').value='slow';f.submit('composer');await f.tick();
  f.w.document.querySelector('.session-rename').click();
  const input=f.w.document.querySelector('.session-rename-form input');input.value='新会话';input.setSelectionRange(1,2);
  f.events[0]({kind:'progress',text:'working'});
  assert.equal(f.w.document.activeElement,input);assert.equal(input.value,'新会话');assert.equal(input.selectionStart,1);
  f.w.document.querySelector('[aria-label="保存名称"]').click();await f.tick();
  f.finishRequest();await f.tick();
  assert.equal(f.saved().sessions[0].title,'新会话');
  f.$('input').value='must not rename';f.submit('composer');await f.tick();
  assert.equal(f.saved().sessions[0].title,'新会话');
  const restored=await fixture({workspace:f.saved()});t.after(()=>restored.dom.window.close());
  restored.$('input').value='still custom';restored.submit('composer');await restored.tick();
  assert.equal(restored.saved().sessions[0].title,'新会话');
});
