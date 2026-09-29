/* Settings drafts contain keys only until save/reload; saved keys never come back. */
let settingsView = null;
let selectedModel = null;
let pendingKeys = new Map();
let settingsLoaded = false;
// Small curated shortcuts, not a claim that the account has access to every model.
const providerPresets = {
  deepseek: { label:'DeepSeek', endpoint:'https://api.deepseek.com/chat/completions', models:[['deepseek-flash','DeepSeek Flash'],['deepseek-v4-pro','DeepSeek V4 Pro']] },
  openai: { label:'OpenAI', endpoint:'https://api.openai.com/v1/chat/completions', models:[['gpt-5-mini','GPT-5 Mini'],['gpt-4.1','GPT-4.1']] },
};
function providerFor(endpoint) {
  try {
    const url = new URL(endpoint);
    if (url.protocol !== 'https:' || url.port || url.search || url.hash || url.username || url.password) return 'custom';
    if (url.hostname === 'api.deepseek.com' && ['/chat/completions','/v1/chat/completions'].includes(url.pathname)) return 'deepseek';
    if (url.hostname === 'api.openai.com' && url.pathname === '/v1/chat/completions') return 'openai';
  } catch { /* Keep incomplete custom URLs editable. */ }
  return 'custom';
}
function renderProvider() {
  const preset = providerPresets[$('provider-preset').value];
  $('endpoint-field').hidden = Boolean(preset);
  $('model-choice-field').hidden = !preset;
  $('provider-help').textContent = preset ? `${preset.label} 官方地址已填好。选择模型并填入该服务的 API Key；实际可用模型取决于你的账号。` : '连接其他服务或本机模型：填写服务地址和模型名称。本机免密服务可不填 API Key。';
  $('model-preset').replaceChildren();
  for (const [value, label] of preset?.models || []) selectOption($('model-preset'), value, label);
  selectOption($('model-preset'), 'custom', '其他模型');
  const names = $('connection-models').value.trim();
  $('model-preset').value = preset?.models.some(([value]) => value === names) ? names : 'custom';
  $('custom-model-field').hidden = Boolean(preset) && $('model-preset').value !== 'custom';
}

function settingsNotice(text, error = false, placement = 'top') {
  const notice = $('settings-status');
  const anchor = placement === 'test' ? $('test-model').parentElement
    : placement === 'save' ? document.querySelector('.settings-save')
    : document.querySelector('.settings-tabs');
  if (placement === 'save') anchor.before(notice); else anchor.after(notice);
  notice.textContent = text;
  notice.hidden = !text;
  notice.classList.toggle('error', error);
}
function applyAppearance(value) { document.documentElement.dataset.theme = value; }
function selectOption(select, value, label) {
  const option = document.createElement('option'); option.value = value; option.textContent = label; select.append(option);
}
function currentModel() { return settingsView?.config.models.choices.find(m => m.id === selectedModel); }
function currentConnection() { return settingsView?.config.models.connections.find(c => c.id === currentModel()?.connection_id); }
function storeModel() {
  const model = currentModel(); let connection = currentConnection();
  if (!model || !connection) return;
  const catalog = settingsView.config.models;
  const endpoint = $('connection-endpoint').value.trim();
  let env = $('connection-env').value.trim();
  const key = $('connection-key').value;
  const siblings = catalog.choices.some(m => m.id !== model.id && m.connection_id === connection.id);
  const sharedCredential = catalog.choices.some(m => m.id !== model.id && catalog.connections.some(c => c.id === m.connection_id && c.endpoint === endpoint && c.api_key_env === env));
  if (siblings && (endpoint !== connection.endpoint || env !== connection.api_key_env || key.trim())) {
    connection = {...connection, id:crypto.randomUUID()};
    catalog.connections.push(connection); model.connection_id = connection.id;
  }
  // Do not overwrite a sibling's endpoint-bound credential or copy saved secrets.
  if (key.trim() && (!env || sharedCredential)) env = `DAO_MODEL_${(sharedCredential ? crypto.randomUUID() : connection.id).replaceAll('-', '_')}`;
  if (connection.endpoint !== endpoint || connection.api_key_env !== env) settingsView.credential_available = settingsView.credential_available.filter(id => id !== connection.id);
  connection.endpoint = endpoint; connection.api_key_env = env;
  $('connection-env').value = env;
  model.name = $('connection-models').value.trim();
  if (key.trim()) pendingKeys.set(model.id, {connection_id:connection.id,key,persist:$('persist-key').checked});
  else pendingKeys.delete(model.id);
}
function renderActiveModels() {
  const catalog = settingsView.config.models;
  $('active-model').replaceChildren(); selectOption($('active-model'), '', '暂不使用模型');
  for (const model of catalog.choices) selectOption($('active-model'), model.id, model.name || '未命名模型');
  if (!catalog.choices.some(m => m.id === catalog.active)) catalog.active = null;
  $('active-model').value = catalog.active || '';
}
function renderModels() {
  $('model-list').replaceChildren();
  for (const model of settingsView.config.models.choices) selectOption($('model-list'), model.id, model.name || '未命名模型');
  $('model-list').value = selectedModel || '';
  const model = currentModel(), connection = currentConnection();
  $('model-fields').hidden = !model;
  $('remove-model').disabled = !model;
  if (model && connection) {
    $('connection-endpoint').value = connection.endpoint;
    $('connection-env').value = connection.api_key_env;
    $('connection-models').value = model.name;
    const key = pendingKeys.get(model.id);
    $('connection-key').value = key?.key || '';
    $('persist-key').checked = key?.persist ?? true;
    $('credential-state').textContent = settingsView.credential_available.includes(connection.id) ? '已有可用凭据 · 留空保留' : '尚未设置 API Key · 本机免密模型可留空';
    $('provider-preset').value = providerFor(connection.endpoint); renderProvider();
  }
  renderActiveModels();
}
function renderDirectories() {
  for (const [id, field] of [['read-roots','read_roots'],['write-roots','write_roots']]) {
    const list = $(id); list.replaceChildren();
    settingsView.config[field].forEach((path,index) => {
      const row = document.createElement('li'), label = document.createElement('span'), remove = document.createElement('button');
      label.textContent = path; label.title = path; remove.type = 'button'; remove.textContent = '移除'; remove.setAttribute('aria-label', `移除目录：${path}`);
      remove.addEventListener('click', () => { if (busy) return; settingsView.config[field].splice(index,1); renderDirectories(); });
      row.append(label,remove); list.append(row);
    });
    if (!list.children.length) { const row = document.createElement('li'); row.textContent = '尚未添加目录'; list.append(row); }
  }
}
function directoryKey(path) { return path.replaceAll('\\','/').replace(/\/+$/, '').toLowerCase(); }
async function addDirectory(field) {
  if (!api || busy || !settingsView) return;
  setBusy(true);
  try {
    const path = await api.invoke('choose_directory');
    if (path && !settingsView.config[field].some(p => directoryKey(p) === directoryKey(path))) { settingsView.config[field].push(path); renderDirectories(); settingsNotice('目录已添加，保存后生效。'); }
  } catch (error) { settingsNotice(`无法选择目录：${error}`, true); }
  finally { setBusy(false); }
}
function accessDescription() {
  $('access-description').textContent = $('access-mode').value === 'full'
    ? '可指定当前系统用户能访问的本机目录。下面的查找目录仅作为搜索起点；留空使用下载、文档和桌面，不自动扫描全盘。'
    : '仅允许在下面的目录中查找。CLI 可写目录也会包含在读取范围内。';
}
async function loadSettings() {
  if (!api || busy) return;
  try {
    settingsView = await api.invoke('get_settings'); settingsLoaded = true;
    pendingKeys.clear(); selectedModel = settingsView.config.models.choices[0]?.id || null;
    $('access-mode').value = settingsView.config.access_mode;
    renderDirectories();
    $('appearance').value = settingsView.config.appearance;
    applyAppearance(settingsView.config.appearance);
    renderModels(); accessDescription(); settingsNotice('');
  } catch (error) { settingsNotice(String(error), true); }
}
$('model-list').addEventListener('change', () => { storeModel(); selectedModel = $('model-list').value; renderModels(); });
$('add-model').addEventListener('click', () => {
  if (!settingsView || busy) return;
  storeModel(); const id = crypto.randomUUID(), modelId = crypto.randomUUID();
  settingsView.config.models.connections.push({id,label:'模型配置',endpoint:providerPresets.deepseek.endpoint,api_key_env:`DAO_MODEL_${id.replaceAll('-', '_')}`});
  settingsView.config.models.choices.push({id:modelId,connection_id:id,name:'deepseek-flash'});
  selectedModel = modelId; renderModels(); settingsNotice('填写模型信息，测试后保存；默认模型可在上方选择。');
});
$('remove-model').addEventListener('click', () => {
  if (!settingsView || busy) return;
  const catalog = settingsView.config.models, connectionId = currentModel()?.connection_id;
  catalog.choices = catalog.choices.filter(m => m.id !== selectedModel);
  if (!catalog.choices.some(m => m.connection_id === connectionId)) catalog.connections = catalog.connections.filter(c => c.id !== connectionId);
  pendingKeys.delete(selectedModel); selectedModel = catalog.choices[0]?.id || null;
  renderModels(); settingsNotice('模型将在保存后移除；已保存的系统凭据不会被删除。');
});
$('provider-preset').addEventListener('change', () => {
  if (!currentModel()) return;
  const preset = providerPresets[$('provider-preset').value];
  $('connection-endpoint').value = preset?.endpoint || '';
  $('connection-models').value = preset?.models[0][0] || '';
  $('connection-env').value = ''; $('connection-key').value = ''; pendingKeys.delete(selectedModel);
  storeModel(); renderModels(); settingsNotice('提供方已切换，请填写对应模型信息。保存后生效。');
});
function updateModelChoices() {
  storeModel(); renderActiveModels();
  const option = [...$('model-list').options].find(o => o.value === selectedModel);
  if (option) option.textContent = currentModel()?.name || '未命名模型';
}
$('add-read-root').addEventListener('click', () => addDirectory('read_roots'));
$('add-write-root').addEventListener('click', () => addDirectory('write_roots'));
$('model-preset').addEventListener('change', () => {
  const custom = $('model-preset').value === 'custom';
  $('custom-model-field').hidden = !custom;
  if (!custom) { $('connection-models').value = $('model-preset').value; updateModelChoices(); }
});
$('connection-models').addEventListener('change', updateModelChoices);
$('active-model').addEventListener('change', () => { settingsView.config.models.active = $('active-model').value || null; });
$('test-model').addEventListener('click', async () => {
  if (!api || busy || !settingsView) return;
  storeModel(); const connection = currentConnection();
  const model = currentModel();
  if (!model) { settingsNotice('请先填写模型名称。', true, 'test'); return; }
  setBusy(true); settingsNotice('正在验证连接与工具调用…', false, 'test');
  try {
    const report = await api.invoke('test_connection', { request:{ model:{ endpoint:connection.endpoint, api_key_env:connection.api_key_env, model:model.name }, key:pendingKeys.get(model.id)?.key || null } });
    settingsNotice(`连接与工具调用通过，${report.elapsed_ms} ms。尚未保存；修改任何连接信息后需要重新测试。`, false, 'test');
  } catch (error) { settingsNotice(String(error), true, 'test'); }
  finally { setBusy(false); }
});
$('settings-form').addEventListener('submit', async (event) => {
  event.preventDefault(); if (!api || busy || !settingsView) return;
  storeModel(); renderActiveModels();
  const config = settingsView.config;
  config.access_mode = $('access-mode').value; config.appearance = $('appearance').value;
  setBusy(true);
  try {
    settingsView = await api.invoke('save_settings', { update:{ expected_revision:settingsView.revision, config, keys:[...pendingKeys.values()] } });
    pendingKeys.clear(); $('connection-key').value = '';
    applyAppearance(settingsView.config.appearance); renderModels();
    invalidateContexts();
    await init(); settingsNotice('设置已保存。历史已保留，模型上下文和旧候选已失效。', false, 'save');
  } catch (error) { settingsNotice(String(error), true, 'save'); }
  finally { setBusy(false); }
});
$('reload-settings').addEventListener('click', loadSettings);
$('access-mode').addEventListener('change', accessDescription);
document.querySelectorAll('[data-settings]').forEach((button) => button.addEventListener('click', () => {
  for (const section of ['models','access','general']) $(`settings-${section}`).hidden = section !== button.dataset.settings;
  document.querySelectorAll('[data-settings]').forEach((other) => other.setAttribute('aria-pressed', String(other === button)));
}));
document.addEventListener('pagechange', (event) => { if (event.detail === 'settings' && !settingsLoaded) loadSettings(); });
loadSettings();
