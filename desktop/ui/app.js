/* All model text and file paths are rendered as text, never HTML. */
const $ = (id) => document.getElementById(id);
const api = window.__TAURI__?.core;
let busy = false;
let confirmationId = null;
let currentPage = 'home';
function showPage(page) {
  currentPage = page;
  for (const name of ['home', 'files', 'settings']) $(`page-${name}`).hidden = name !== page;
  $('entry-footer').hidden = page === 'settings';
  $('page-title').textContent = { home:'我的电脑', files:'文件与对话', settings:'设置' }[page];
  document.querySelectorAll('[data-page]').forEach((button) => {
    if (button.dataset.page === page) button.setAttribute('aria-current', 'page'); else button.removeAttribute('aria-current');
  });
  document.dispatchEvent(new CustomEvent('pagechange', { detail:page }));
}
document.querySelectorAll('[data-page]').forEach((button) => button.addEventListener('click', () => showPage(button.dataset.page)));

function pathText(path) { return String(path).replace(/^\\\\\?\\/, ''); }
function nameOf(path) { return pathText(path).split(/[\\/]/).pop() || pathText(path); }
function setBusy(value) {
  busy = value;
  for (const id of ['send', 'input', 'mode', 'reset']) $(id).disabled = value;
  $('cancel').hidden = !value;
  $('settings-cancel').hidden = !value;
  document.querySelectorAll('.settings-controls input,.settings-controls select,.settings-controls textarea,.settings-controls button,#refresh-home,#explain-home').forEach((node) => { node.disabled = value; });
  document.querySelectorAll('.file-row button,.suggestions button').forEach((button) => { button.disabled = value; });
}
function message(text, role = 'assistant', error = false) {
  $('welcome').hidden = true;
  const block = document.createElement('div'); block.className = `message ${role}${error ? ' error' : ''}`;
  const speaker = document.createElement('div'); speaker.className = 'speaker'; speaker.textContent = role === 'user' ? '你' : role === 'receipt' ? '本地回执' : 'DAO';
  const body = document.createElement('div'); body.textContent = text;
  block.append(speaker, body); $('conversation').append(block);
  block.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
}
function candidates(items) {
  $('candidate-list').replaceChildren(); $('candidate-panel').hidden = !items.length;
  $('candidate-count').textContent = `${items.length} 项`;
  for (const item of items) {
    const row = document.createElement('div'); row.className = 'file-row';
    const icon = document.createElement('div'); icon.className = 'file-icon'; icon.textContent = item.identity.directory ? '目录' : (nameOf(item.path).split('.').pop() || 'FILE').slice(0, 5).toUpperCase();
    const detail = document.createElement('div'); detail.className = 'file-detail';
    const name = document.createElement('div'); name.className = 'file-name'; name.textContent = nameOf(item.path);
    const path = document.createElement('div'); path.className = 'file-path'; path.textContent = pathText(item.path);
    detail.append(name, path);
    const open = document.createElement('button'); open.textContent = '打开 ↗'; open.disabled = busy;
    open.addEventListener('click', () => run('open', item.id));
    row.append(icon, detail, open); $('candidate-list').append(row);
  }
}
function closeConfirmation() { confirmationId = null; $('confirmation').hidden = true; }
function onEvent(event) {
  if (event.kind === 'progress') $('activity').textContent = event.text;
  if (event.kind === 'result' && event.capability === 'file_search') candidates(event.value.items || []);
  if (event.kind === 'result' && event.capability === 'file_open') {
    message(event.value.message || '打开请求已处理。', 'receipt');
  }
  if (event.kind === 'confirm_open') {
    showPage('files');
    confirmationId = event.request_id;
    $('confirm-name').textContent = nameOf(event.object.path);
    $('confirm-path').textContent = pathText(event.object.path);
    $('confirmation').hidden = false;
    $('approve').disabled = false; $('reject').disabled = false;
    $('confirmation').scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    $('reject').focus();
    $('activity').textContent = '等待你的确认…';
  }
  if (event.kind === 'confirmation_closed' && confirmationId === event.request_id) closeConfirmation();
}
async function run(kind, input = '') {
  if (busy || !api) return;
  showPage('files');
  setBusy(true); closeConfirmation();
  if (kind === 'say' || kind === 'search') { message(input, 'user'); $('input').value = ''; }
  if (kind === 'explain') message('解释这次电脑资源快照', 'user');
  $('activity').textContent = '正在处理…';
  const channel = new api.Channel(); channel.onmessage = onEvent;
  try {
    const reply = await api.invoke('perform', { kind, input, onEvent: channel });
    if (kind === 'reset' && !reply.error) {
      document.querySelectorAll('.message').forEach((node) => node.remove()); $('welcome').hidden = false;
    } else if (kind !== 'open' || reply.error) message(reply.message, 'assistant', reply.error);
    candidates(reply.items);
    $('activity').textContent = reply.error ? '这次没有完成，可以调整后再试。' : '准备就绪';
  } catch (error) { message(String(error), 'assistant', true); $('activity').textContent = '请求未完成'; }
  finally { closeConfirmation(); setBusy(false); $('input').focus(); }
}
async function answer(approved) {
  if (!confirmationId || !api) return;
  $('approve').disabled = true; $('reject').disabled = true;
  try { await api.invoke('confirm_open', { requestId: confirmationId, approved }); }
  catch (error) { message(String(error), 'assistant', true); }
  finally { closeConfirmation(); }
}
$('composer').addEventListener('submit', (event) => { event.preventDefault(); const input = $('input').value.trim(); if (input) run($('mode').value, input); });
$('input').addEventListener('keydown', (event) => { if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) { event.preventDefault(); $('composer').requestSubmit(); } });
$('reset').addEventListener('click', () => run('reset'));
$('cancel').addEventListener('click', () => { if (api) api.invoke('cancel').catch((error) => message(String(error), 'assistant', true)); });
$('settings-cancel').addEventListener('click', () => $('cancel').click());
$('approve').addEventListener('click', () => answer(true)); $('reject').addEventListener('click', () => answer(false));
document.querySelectorAll('[data-prompt]').forEach((button) => button.addEventListener('click', () => { $('input').value = button.dataset.prompt; $('input').focus(); }));
async function init() {
  if (!api) { $('activity').textContent = '界面预览：文件与模型能力仅在桌面应用中可用。'; $('model-label').textContent = '静态界面预览'; setBusy(true); $('cancel').hidden = true; return; }
  try {
    const info = await api.invoke('session_info');
    $('model-label').textContent = info.model_error ? '尚未连接模型' : info.model;
    $('model-dot').classList.toggle('offline', Boolean(info.model_error));
    $('roots').replaceChildren();
    for (const root of info.roots) { const li = document.createElement('li'); li.textContent = pathText(root); $('roots').append(li); }
    $('access-label').textContent = info.access_mode === 'full' ? '完全访问' : '安全模式';
    $('scope-note').textContent = info.access_mode === 'full' ? '以上为默认查找位置；可以指定其他本机目录。操作仍需确认。' : '只在已选择的目录中查找。';
    const notes = [];
    if (info.model_error) notes.push(info.model_error);
    if (!info.roots.length && info.access_mode !== 'full') notes.push('还没有查找目录。请在设置 → 访问权限中选择目录。');
    $('config-notice').hidden = !notes.length;
    if (notes.length) { $('config-notice').textContent = notes.join('\n'); $('config-notice').hidden = false; }
  } catch (error) { $('config-notice').textContent = String(error); $('config-notice').hidden = false; $('activity').textContent = '配置未能加载，请修正后重启入口。'; }
}
init();
