import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import { Script } from 'node:vm';

const clone = (value) => JSON.parse(JSON.stringify(value));
const file = { id:'internal-object-id', path:'C:\\fixture\\合同 <script>.txt', identity:{ directory:false, size:12 } };
async function fixture() {
  const dom = new JSDOM(await readFile(new URL('../ui/index.html', import.meta.url), 'utf8'), { runScripts:'outside-only', url:'http://tauri.localhost/' });
  const w = dom.window; w.HTMLElement.prototype.scrollIntoView = function () {};
  const calls = []; let current = { revision:'original', credential_available:[], config:{ read_roots:['C:\\fixture'], write_roots:[], access_mode:'restricted', appearance:'light', model:null, models:{ connections:[{id:'local',label:'Local',endpoint:'http://127.0.0.1:12345/v1/chat/completions',api_key_env:''}], choices:[{id:'fixture',connection_id:'local',name:'fixture-model'}],active:'fixture' } } };
  let pending = null;
  w.__TAURI__ = { core:{ Channel:class {}, invoke:async (command, args) => {
    calls.push({command,args:clone(args || {})});
    if (command === 'session_info') return { roots:current.config.read_roots, model:'fixture-model', model_error:null, access_mode:current.config.access_mode };
    if (command === 'get_settings') return clone(current);
    if (command === 'save_settings') { assert.equal(args.update.expected_revision, current.revision); current = { revision:'saved', credential_available:[],config:clone(args.update.config) }; return clone(current); }
    if (command === 'test_connection') return { elapsed_ms:20,total_tokens:null };
    if (command === 'computer_overview') return {system:{cpu_percent:12.5,memory_used_bytes:1024,memory_total_bytes:2048,memory_available_bytes:1024,disks:[{mount:'C:\\',total_bytes:4096,available_bytes:2048}]},device:{host:'Fixture computer',os:'Fixture OS',uptime_seconds:3600,architecture:'x86_64'},logical_cpus:2,observed_at:'2026-09-21T10:00:00Z',network:{adapters:[],limits:'Fixture counters'},applications:{items:[{name:'<img src=x onerror=alert(1)>',version:'1'}],available:true,partial:true,limits:'Partial fixture'}};
    if (command === 'perform') {
      if (args.input === 'slow') return new Promise((resolve) => { pending = resolve; });
      if (args.kind === 'reset') return {message:'reset',error:false,items:[]};
      const items = args.input === 'absent' ? [] : [file];
      args.onEvent.onmessage({kind:'result',capability:'file_search',value:{items}});
      return {message:'fixture response',error:false,items};
    }
    if (command === 'cancel') { pending?.({message:'本轮已取消',error:true,items:[]}); return; }
    throw Error(`Unexpected command ${command}`);
  } } };
  for (const script of ['app.js','layout.js','settings.js','home.js']) new Script(await readFile(new URL(`../ui/${script}`, import.meta.url), 'utf8'), { filename:script }).runInContext(dom.getInternalVMContext());
  const tick = async () => { for(let i=0;i<5;i++) await new Promise((resolve) => setTimeout(resolve,0)); };
  await tick();
  const $ = (id) => w.document.getElementById(id);
  const change = (id,value) => { $(id).value=value; $(id).dispatchEvent(new w.Event('change',{bubbles:true})); };
  const submit = (id) => $(id).dispatchEvent(new w.Event('submit',{bubbles:true,cancelable:true}));
  return {dom,w,$,calls,tick,change,submit};
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
  assert.equal(f.w.localStorage.getItem('dao.sidebarWidth'),'360');
  f.w.innerWidth=680;f.w.dispatchEvent(new f.w.Event('resize'));
  assert.equal(f.$('sidebar-resize').getAttribute('aria-valuenow'),'220');
  assert.equal(f.$('applications').dataset.view,'cards');
  f.$('app-filter').value='no match';f.$('app-filter').dispatchEvent(new f.w.Event('input'));
  f.w.document.querySelector('[data-app-view="list"]').click();
  assert.equal(f.$('applications').dataset.view,'list');
  assert.match(f.$('applications').textContent,/没有匹配/);
  assert.equal(f.w.localStorage.getItem('dao.appView'),'list');
});
