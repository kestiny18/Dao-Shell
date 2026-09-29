/* Persist only visible conversation data. Runtime candidates and confirmations stay in memory. */
const $ = (id) => document.getElementById(id);
const api = window.__TAURI__?.core;
let busy = false;
let currentPage = 'home';
let renderedTab = 'home';
let workspace = {version:1,sessions:[],tabs:['home'],active:'home',sidebar_width:205,collapsed:false,home_draft:''};
const runtime = new Map();
let activeRequest = null;
let ready = false;
let mutatingWorkspace = false;
let persistenceBlocked = false;
let saveLoop = null;
let pendingSave = null;
const pageScroll = new Map();
function session(id = workspace.active) { return workspace.sessions.find(s => s.id === id); }
function state(id) { if (!runtime.has(id)) runtime.set(id,{items:[],confirmation:null,activity:'准备就绪',contextLost:false}); return runtime.get(id); }
function pathText(path) { return String(path).replace(/^\\\\\?\\/, ''); }
function nameOf(path) { return pathText(path).split(/[\\/]/).pop() || pathText(path); }
function persistenceNotice(text) { $('workspace-status').textContent = text; $('workspace-status').hidden = !text; }
function persist() {
  if (!ready || !api) return Promise.resolve();
  if (persistenceBlocked) return Promise.reject(Error('工作区未恢复，不能保存。请关闭应用，备份并移走配置目录中的 workspace.json 后重启。'));
  pendingSave = JSON.parse(JSON.stringify(workspace));
  if (!saveLoop) {
    saveLoop = (async () => {
      while (pendingSave) {
        const value = pendingSave; pendingSave = null;
        try { await api.invoke('save_workspace', {workspace:value}); persistenceNotice(''); }
        catch (error) { pendingSave = null; persistenceNotice(`尚未保存：${error}。请检查磁盘和配置目录权限，然后点击“重试保存”。`); throw error; }
      }
    })().finally(() => { saveLoop = null; });
  }
  return saveLoop;
}
function saveSoon() { persist().catch(() => {}); }
function captureDraft() {
  // The composer belongs to the rendered tab, even during workspace mutations.
  const s = session(renderedTab);
  if (s) s.draft = $('input').value;
  else if (renderedTab === 'home') workspace.home_draft = $('input').value;
}
function button(text, label, action) {
  const node = document.createElement('button'); node.type = 'button'; node.textContent = text; node.setAttribute('aria-label',label); node.title = label; node.addEventListener('click',action); return node;
}
let renameEditor = null;
function startRename(id) {
  if (mutatingWorkspace || renameEditor?.saving) return;
  const item = session(id); if (!item) return;
  const form = document.createElement('form'); form.className = 'session-rename-form';
  const input = document.createElement('input'); input.value = item.title; input.maxLength = 512;
  input.setAttribute('aria-label','会话名称'); input.autocomplete = 'off';
  const error = document.createElement('small'); error.className = 'rename-error'; error.setAttribute('role','status');
  const save = button('✓','保存名称',() => form.requestSubmit());
  const cancel = button('×','取消重命名',cancelRename);
  form.append(input,save,cancel,error);
  renameEditor = {id,form,input,error,saving:false};
  form.addEventListener('submit',event => {event.preventDefault(); saveRename();});
  input.addEventListener('keydown',event => {
    if (event.isComposing || event.keyCode === 229) { if (event.key === 'Enter') event.preventDefault(); return; }
    if (event.key === 'Escape') {event.preventDefault(); cancelRename();}
  });
  renderNavigation(); input.focus(); input.select();
}
function cancelRename() {
  if (!renameEditor || renameEditor.saving) return;
  const id = renameEditor.id; renameEditor = null; renderNavigation();
  document.querySelector(`[data-rename-id="${id}"]`)?.focus();
}
async function saveRename() {
  const editor = renameEditor; if (!editor || editor.saving) return;
  const title = editor.input.value.trim();
  // Workspace validation counts UTF-8 bytes, not JavaScript code units.
  if (!title || new TextEncoder().encode(title).length > 512) {
    editor.error.textContent = title ? '名称过长，请控制在 512 字节以内（约 170 个汉字）。' : '请输入会话名称。'; return;
  }
  const item = session(editor.id); if (!item) {cancelRename(); return;}
  const previous = item.title; const autoTitle = state(item.id).autoTitle;
  editor.saving = true; editor.error.textContent = '正在保存…';
  editor.form.querySelectorAll('input,button').forEach(n => {n.disabled = true;});
  item.title = title; state(item.id).autoTitle = false;
  try {
    await persist(); renameEditor = null;
  } catch (error) {
    item.title = previous; state(item.id).autoTitle = autoTitle;
    editor.error.textContent = `名称未保存：${error}。请重试。`;
  } finally {
    editor.saving = false; editor.form.querySelectorAll('input,button').forEach(n => {n.disabled = false;});
    renderNavigation();
    if (workspace.active === item.id) $('page-title').textContent = item.title;
    if (renameEditor === editor) editor.input.focus();
    else document.querySelector(`[data-rename-id="${item.id}"]`)?.focus();
  }
}
function renderNavigation() {
  const focusedNode = document.activeElement;
  const focused = focusedNode?.getAttribute('aria-label');
  const editingFocus = renameEditor?.form.contains(focusedNode);
  $('session-list').replaceChildren();
  for (const s of workspace.sessions) {
    const row = document.createElement('div'); row.className = 'session-row';
    const open = button(s.title, `打开会话：${s.title}`, () => selectTab(s.id)); open.dataset.page = 'files'; open.className = 'session-name';
    if (s.id === workspace.active) open.setAttribute('aria-current','page');
    const rename = button('✎',`重命名会话：${s.title}`,() => startRename(s.id)); rename.className = 'session-rename'; rename.dataset.renameId = s.id;
    if (renameEditor?.id === s.id) row.append(renameEditor.form);
    else row.append(open,rename);
    $('session-list').append(row);
  }
  $('workspace-tabs').replaceChildren();
  for (const id of workspace.tabs) {
    const title = id === 'home' ? '概览' : id === 'settings' ? '设置' : session(id)?.title || '会话';
    const tab = document.createElement('div'); tab.className = 'workspace-tab';
    const open = button(title,`切换到${title}`,() => selectTab(id)); open.setAttribute('role','tab'); open.setAttribute('aria-selected',String(id === workspace.active));
    if (id === workspace.active) tab.classList.add('selected');
    tab.append(open);
    if (id !== 'home') { const close = button('×',`关闭标签：${title}`,() => closeTab(id)); close.className = 'tab-close'; tab.append(close); }
    $('workspace-tabs').append(tab);
  }
  if (editingFocus) focusedNode.focus({preventScroll:true});
  if (focused && !editingFocus) { const target = [...document.querySelectorAll('button[aria-label]')].find(n => n.getAttribute('aria-label') === focused); target?.focus({preventScroll:true}); }
}
function renderMessage(m) {
  const block = document.createElement('div'); block.className = `message ${m.role}${m.error ? ' error' : ''}`;
  const speaker = document.createElement('div'); speaker.className = 'speaker'; speaker.textContent = m.role === 'user' ? '你' : m.role === 'receipt' ? '本地回执' : 'DAO';
  const body = document.createElement('div'); body.textContent = m.text; block.append(speaker,body); $('conversation').append(block);
}
function candidates(items) {
  $('candidate-list').replaceChildren(); $('candidate-panel').hidden = !items.length; $('candidate-count').textContent = `${items.length} 项`;
  for (const item of items) {
    const row = document.createElement('div'); row.className = 'file-row';
    const detail = document.createElement('div'); detail.className = 'file-detail';
    const name = document.createElement('div'); name.className = 'file-name'; name.textContent = nameOf(item.path);
    const path = document.createElement('div'); path.className = 'file-path'; path.textContent = pathText(item.path); detail.append(name,path);
    const open = button('打开 ↗',`打开 ${nameOf(item.path)}`,() => run('open',item.id)); open.disabled = busy;
    row.append(detail,open); $('candidate-list').append(row);
  }
}
function renderCurrent() {
  const s = session(); const st = s ? state(s.id) : null;
  currentPage = s ? 'files' : workspace.active;
  for (const name of ['home','files','settings']) $(`page-${name}`).hidden = name !== currentPage;
  $('entry-footer').hidden = currentPage === 'settings';
  $('page-title').textContent = s?.title || (currentPage === 'home' ? '概览' : '设置');
  document.querySelectorAll('.message').forEach(n => n.remove());
  for (const m of s?.messages || []) renderMessage(m);
  $('welcome').hidden = Boolean(s?.messages.length);
  $('context-notice').hidden = !st?.contextLost;
  candidates(st?.items || []);
  $('input').value = s?.draft ?? (currentPage === 'home' ? workspace.home_draft : '');
  renderedTab = workspace.active;
  $('activity').textContent = st?.activity || (activeRequest ? '另一会话正在处理，可切换查看或停止。' : '准备就绪');
  const c = st?.confirmation;
  $('confirmation').hidden = !c;
  if (c) { $('confirm-name').textContent = nameOf(c.object.path); $('confirm-path').textContent = pathText(c.object.path); }
  $('approve').disabled = Boolean(c?.answering); $('reject').disabled = Boolean(c?.answering);
  setBusy(busy); renderNavigation();
}
function selectTab(id) {
  if (!['home','settings'].includes(id) && !session(id)) return;
  captureDraft();
  const scroller = document.querySelector('.content-scroll'); pageScroll.set(workspace.active,scroller.scrollTop);
  if (!workspace.tabs.includes(id)) workspace.tabs.push(id);
  workspace.active = id; renderCurrent(); scroller.scrollTop = pageScroll.get(id) || 0;
  document.dispatchEvent(new CustomEvent('pagechange',{detail:currentPage})); saveSoon();
}
document.querySelectorAll('[data-page]').forEach(b => b.addEventListener('click',() => selectTab(b.dataset.page)));
function setBusy(value) {
  busy = value;
  $('send').disabled = value || !ready; $('input').disabled = !ready;
  $('reset').disabled = !ready;
  $('cancel').hidden = !value; $('settings-cancel').hidden = !value;
  document.querySelectorAll('.settings-controls input,.settings-controls select,.settings-controls textarea,.settings-controls button,#refresh-home,#explain-home').forEach(n => {n.disabled = value;});
  document.querySelectorAll('.file-row button').forEach(n => {n.disabled = value;});
}
function message(text, role = 'assistant', error = false, id = workspace.active) {
  const s = session(id); if (!s) return;
  s.messages.push({text:String(text).slice(0,64000),role,error});
  if (workspace.active === id) { $('welcome').hidden = true; renderMessage(s.messages.at(-1)); }
  saveSoon();
}
function closeConfirmation(id = workspace.active) { if (runtime.has(id)) state(id).confirmation = null; if (workspace.active === id) $('confirmation').hidden = true; }
function invalidateContexts() {
  for (const s of workspace.sessions) { const st = state(s.id); st.items = []; st.confirmation = null; st.contextLost = true; }
  renderCurrent();
}
function onEvent(event, request) {
  if (activeRequest !== request || !session(request.sessionId) || event.session_id !== request.sessionId || event.execution_id !== request.id) return;
  const st = state(request.sessionId);
  if (event.kind === 'progress') st.activity = event.text;
  if (event.kind === 'context_reset') {
    for (const item of event.configuration_changed ? workspace.sessions : [session(request.sessionId)]) {
      const previous = state(item.id); previous.items = []; previous.contextLost = item.messages.length > (item.id === request.sessionId ? 1 : 0);
    }
  }
  if (event.kind === 'result' && event.capability === 'file_search') {
    st.items = event.value.items || [];
    if (st.items.length) message('搜索结果（历史记录；操作前请重新搜索）：\n' + st.items.map(item => pathText(item.path)).join('\n'),'receipt',false,request.sessionId);
  }
  if (event.kind === 'result' && event.capability === 'file_open') message(event.value.message || '打开请求已处理。','receipt',false,request.sessionId);
  if (event.kind === 'confirm_open' && !request.stopping) { st.confirmation = {...event,executionId:request.id}; st.activity = '等待你的确认…'; }
  if (event.kind === 'confirmation_closed' && st.confirmation?.request_id === event.request_id) st.confirmation = null;
  if (workspace.active === request.sessionId) { captureDraft(); renderCurrent(); }
}
let creating = false;
async function newSession() {
  if (!ready || creating || mutatingWorkspace) return null;
  creating = true;
  try {
    const id = crypto.randomUUID(); if (api) await api.invoke('register_session',{sessionId:id});
    captureDraft(); workspace.sessions.push({id,title:'新会话',draft:'',messages:[]}); state(id).autoTitle = true; selectTab(id); $('input').focus(); return id;
  } catch (error) { persistenceNotice(String(error)); return null; }
  finally { creating = false; }
}
async function run(kind,input = '') {
  if (busy || !api || !ready || mutatingWorkspace) return;
  let id = session()?.id;
  if (!id) { id = await newSession(); if (!id || busy) return; workspace.home_draft = ''; }
  const s = session(id); s.draft = ''; $('input').value = '';
  const request = {id:crypto.randomUUID(),sessionId:id,stopping:false}; activeRequest = request;
  setBusy(true); closeConfirmation(id);
  if (kind === 'say' || kind === 'search') { message(input,'user',false,id); if (state(id).autoTitle) { s.title = input.slice(0,60); state(id).autoTitle = false; } }
  if (kind === 'explain') message('解释这次电脑资源快照','user',false,id);
  state(id).activity = '正在处理…'; renderCurrent();
  const channel = new api.Channel(); channel.onmessage = event => onEvent(event,request);
  request.done = (async () => {
    try {
      const reply = await api.invoke('perform',{sessionId:id,requestId:request.id,kind,input,onEvent:channel});
      if (activeRequest !== request || !session(id)) return;
      if (kind !== 'open' || reply.error) message(reply.message,'assistant',reply.error,id);
      state(id).items = request.stopping ? [] : reply.items;
      state(id).activity = request.stopping ? '已停止' : reply.error ? '这次没有完成，可以调整后再试。' : '准备就绪';
    } catch (error) { message(String(error),'assistant',true,id); state(id).activity = '请求未完成'; }
    finally {
      closeConfirmation(id);
      if (activeRequest === request) { activeRequest = null; setBusy(false); captureDraft(); renderCurrent(); saveSoon(); }
    }
  })();
  await request.done;
}
async function stopRequest() {
  const request = activeRequest;
  if (!api) return;
  if (!request) { await api.invoke('cancel'); return; }
  request.stopping = true; closeConfirmation(request.sessionId);
  try { await api.invoke('cancel',{sessionId:request.sessionId,requestId:request.id}); }
  catch (error) { if (activeRequest === request) { await request.done; if (activeRequest === request) throw error; } }
  await request.done;
}
async function answer(approved) {
  const id = workspace.active; const c = state(id).confirmation;
  if (!c || !activeRequest || activeRequest.sessionId !== id || activeRequest.id !== c.executionId || activeRequest.stopping) return;
  c.answering = true; renderCurrent();
  try { await api.invoke('confirm_open',{sessionId:id,executionId:c.executionId,requestId:c.request_id,approved}); }
  catch (error) { message(String(error),'assistant',true,id); }
  finally { closeConfirmation(id); }
}
let decision = null;
function ask(text) {
  if (decision) return Promise.resolve(false);
  document.querySelector('.shell').inert = true;
  $('decision-text').textContent = text; $('decision').hidden = false; $('decision-no').focus();
  return new Promise(resolve => { decision = resolve; });
}
function decide(value) { document.querySelector('.shell').inert = false; $('decision').hidden = true; const resolve = decision; decision = null; resolve?.(value); }
$('decision-no').addEventListener('click',() => decide(false)); $('decision-yes').addEventListener('click',() => decide(true));
document.addEventListener('keydown',event => {
  if (!decision) return;
  if (event.key === 'Escape') decide(false);
  if (event.key === 'Tab') { event.preventDefault(); (document.activeElement === $('decision-no') ? $('decision-yes') : $('decision-no')).focus(); }
});
async function closeTab(id) {
  if (id === 'home' || mutatingWorkspace) return;
  const executing = activeRequest?.sessionId === id;
  if (executing && !await ask('停止当前请求并关闭标签？')) return;
  try {
    if (activeRequest?.sessionId === id) await stopRequest();
    mutatingWorkspace = true; document.querySelector('.shell').inert = true;
    captureDraft();
    const previousTabs = [...workspace.tabs]; const previousActive = workspace.active;
    workspace.tabs = workspace.tabs.filter(t => t !== id);
    if (workspace.active === id) workspace.active = workspace.tabs.at(-1) || 'home';
    // Commit the visible selection before yielding to request callbacks or saves.
    renderCurrent();
    try { await persist(); }
    catch (error) {
      workspace.tabs = previousTabs; workspace.active = previousActive;
      renderCurrent(); throw error;
    }
    renderCurrent();
  } catch (error) { persistenceNotice(String(error)); }
  finally { mutatingWorkspace = false; document.querySelector('.shell').inert = false; }
}
$('composer').addEventListener('submit',event => { event.preventDefault(); const input = $('input').value.trim(); if (input) run('say',input); });
$('input').addEventListener('input',() => { captureDraft(); saveSoon(); });
$('input').addEventListener('keydown',event => { if (event.key === 'Enter' && !event.shiftKey && !event.isComposing && event.keyCode !== 229) { event.preventDefault(); $('composer').requestSubmit(); } });
$('reset').addEventListener('click',newSession);
$('cancel').addEventListener('click',() => stopRequest().catch(error => persistenceNotice(String(error))));
$('settings-cancel').addEventListener('click',() => $('cancel').click());
$('approve').addEventListener('click',() => answer(true)); $('reject').addEventListener('click',() => answer(false));
$('retry-save').addEventListener('click',saveSoon);
document.querySelectorAll('[data-prompt]').forEach(b => b.addEventListener('click',() => { $('input').value = b.dataset.prompt; captureDraft(); saveSoon(); $('input').focus(); }));
async function restoreWorkspace() {
  if (api) {
    try { workspace = await api.invoke('load_workspace'); for (const s of workspace.sessions) state(s.id).contextLost = true; }
    catch (error) { persistenceBlocked = true; persistenceNotice(`恢复失败：${error}。原文件保留；本次内容不会保存。请关闭应用，备份并移走配置目录中的 workspace.json 后重启。`); }
  }
  // Keep legacy workspace files compatible while the collapse control is deferred.
  workspace.collapsed = false;
  ready = true; renderCurrent();
  document.dispatchEvent(new CustomEvent('workspaceready'));
  document.dispatchEvent(new CustomEvent('pagechange',{detail:currentPage}));
  await init();
}
async function init() {
  if (!api) { $('activity').textContent = '界面预览：文件与模型能力仅在桌面应用中可用。'; $('model-label').textContent = '静态界面预览'; setBusy(true); $('cancel').hidden = true; return; }
  try {
    const info = await api.invoke('session_info');
    $('model-label').textContent = info.model_error ? '尚未连接模型' : info.model;
    $('model-dot').classList.toggle('offline', Boolean(info.model_error));
    const notes = [];
    if (info.model_error) notes.push(info.model_error);
    if (!info.roots.length && info.access_mode !== 'full') notes.push('还没有查找目录。请在设置 → 访问权限中选择目录。');
    $('config-notice').hidden = !notes.length;
    if (notes.length) { $('config-notice').textContent = notes.join('\n'); $('config-notice').hidden = false; }
  } catch (error) { $('config-notice').textContent = String(error); $('config-notice').hidden = false; $('activity').textContent = '配置未能加载，请修正后重启入口。'; }
}


restoreWorkspace();

let closingWindow = false;
if (window.__TAURI__?.event) window.__TAURI__.event.listen('workspace-close-requested', async () => {
  if (closingWindow) return;
  closingWindow = true;
  try {
    if (activeRequest) await stopRequest();
    captureDraft();
    if (persistenceBlocked) {
      if (!await ask('工作区恢复失败，本次内容未保存。仍要退出？')) return;
    } else {
      try { await persist(); }
      catch { if (!await ask('保存失败，本次更改可能丢失。仍要退出？')) return; }
    }
    await api.invoke('finish_close');
  } finally { closingWindow = false; }
}).catch(error => persistenceNotice(`无法监听退出事件：${error}`));
