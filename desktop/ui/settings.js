/* Settings drafts contain keys only until save/reload; saved keys never come back. */
let settingsView = null;
let selectedConnection = null;
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
  selectOption($('model-preset'), 'custom', '其他模型 / 多个模型');
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
function currentConnection() { return settingsView?.config.models.connections.find((c) => c.id === selectedConnection); }
function storeConnection() {
  const connection = currentConnection();
  if (!connection) return;
  // Custom services should not require the user to understand credential references.
  if ($('connection-key').value.trim() && !$('connection-env').value.trim()) {
    $('connection-env').value = `DAO_MODEL_${connection.id.replaceAll('-', '_')}`;
  }
  if (connection.endpoint !== $('connection-endpoint').value.trim() || connection.api_key_env !== $('connection-env').value.trim()) {
    settingsView.credential_available = settingsView.credential_available.filter((id) => id !== connection.id);
  }
  connection.label = $('connection-label').value.trim();
  connection.endpoint = $('connection-endpoint').value.trim();
  connection.api_key_env = $('connection-env').value.trim();
  const catalog = settingsView.config.models;
  const previous = catalog.choices.filter((m) => m.connection_id === connection.id);
  const names = [...new Set($('connection-models').value.split('\n').map((x) => x.trim()).filter(Boolean))];
  catalog.choices = catalog.choices.filter((m) => m.connection_id !== connection.id).concat(names.map((name) => ({ id:previous.find((m) => m.name === name)?.id || crypto.randomUUID(), connection_id:connection.id, name })));
  const key = $('connection-key').value;
  if (key.trim()) pendingKeys.set(connection.id, { connection_id:connection.id, key, persist:$('persist-key').checked });
  else pendingKeys.delete(connection.id);
}
function renderActiveModels() {
  const catalog = settingsView.config.models;
  $('active-model').replaceChildren(); selectOption($('active-model'), '', '暂不使用模型');
  for (const model of catalog.choices) {
    const connection = catalog.connections.find((c) => c.id === model.connection_id);
    selectOption($('active-model'), model.id, `${connection?.label || '连接'} / ${model.name}`);
  }
  if (!catalog.choices.some((m) => m.id === catalog.active)) catalog.active = null;
  $('active-model').value = catalog.active || '';
}
function renderConnections() {
  $('connection-list').replaceChildren();
  for (const connection of settingsView.config.models.connections) selectOption($('connection-list'), connection.id, connection.label || '未命名连接');
  $('connection-list').value = selectedConnection || '';
  const connection = currentConnection();
  $('connection-fields').hidden = !connection;
  $('remove-connection').disabled = !connection;
  if (connection) {
    $('connection-label').value = connection.label;
    $('connection-endpoint').value = connection.endpoint;
    $('connection-env').value = connection.api_key_env;
    $('connection-models').value = settingsView.config.models.choices.filter((m) => m.connection_id === connection.id).map((m) => m.name).join('\n');
    const key = pendingKeys.get(connection.id);
    $('connection-key').value = key?.key || '';
    $('persist-key').checked = key?.persist ?? true;
    $('credential-state').textContent = settingsView.credential_available.includes(connection.id) ? '已有可用凭据 · 留空保留' : '尚未设置 API Key · 本机免密服务可留空';
    $('provider-preset').value = providerFor(connection.endpoint);
    renderProvider();
  }
  renderActiveModels();
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
    pendingKeys.clear(); selectedConnection = settingsView.config.models.connections[0]?.id || null;
    $('access-mode').value = settingsView.config.access_mode;
    $('read-roots').value = settingsView.config.read_roots.join('\n');
    $('write-roots').value = settingsView.config.write_roots.join('\n');
    $('appearance').value = settingsView.config.appearance;
    applyAppearance(settingsView.config.appearance);
    renderConnections(); accessDescription(); settingsNotice('');
  } catch (error) { settingsNotice(String(error), true); }
}
$('connection-list').addEventListener('change', () => { storeConnection(); selectedConnection = $('connection-list').value; renderConnections(); });
$('add-connection').addEventListener('click', () => {
  if (!settingsView) return;
  storeConnection(); const id = crypto.randomUUID();
  settingsView.config.models.connections.push({ id, label:'DeepSeek', endpoint:providerPresets.deepseek.endpoint, api_key_env:`DAO_MODEL_${id.replaceAll('-', '_')}` });
  const modelId = crypto.randomUUID();
  settingsView.config.models.choices.push({ id:modelId, connection_id:id, name:'deepseek-flash' });
  if (!settingsView.config.models.active) settingsView.config.models.active = modelId;
  selectedConnection = id; renderConnections(); settingsNotice('填写连接信息、测试连接，再保存。');
});
$('remove-connection').addEventListener('click', () => {
  if (!settingsView) return;
  const catalog = settingsView.config.models;
  catalog.connections = catalog.connections.filter((c) => c.id !== selectedConnection);
  catalog.choices = catalog.choices.filter((m) => m.connection_id !== selectedConnection);
  pendingKeys.delete(selectedConnection); selectedConnection = catalog.connections[0]?.id || null;
  renderConnections(); settingsNotice('连接将在保存后移除；系统中已有凭据不会被删除。');
});
$('provider-preset').addEventListener('change', () => {
  const preset = providerPresets[$('provider-preset').value];
  const catalog = settingsView.config.models;
  const wasActive = !catalog.active || catalog.choices.some((m) => m.id === catalog.active && m.connection_id === selectedConnection);
  $('connection-label').value = preset?.label || '自定义服务';
  $('connection-endpoint').value = preset?.endpoint || '';
  $('connection-models').value = preset?.models[0][0] || '';
  if (preset && !$('connection-env').value) $('connection-env').value = `DAO_MODEL_${selectedConnection.replaceAll('-', '_')}`;
  if (!preset) $('connection-env').value = '';
  $('connection-key').value = ''; pendingKeys.delete(selectedConnection);
  storeConnection();
  if (wasActive) catalog.active = catalog.choices.find((m) => m.connection_id === selectedConnection)?.id || null;
  renderConnections();
  settingsNotice('服务已切换，请填写对应的 API Key。更改只在保存后生效。');
});
function updateModelChoices() {
  const catalog = settingsView.config.models;
  const wasActive = !catalog.active || catalog.choices.some((m) => m.id === catalog.active && m.connection_id === selectedConnection);
  storeConnection();
  if (wasActive && !catalog.choices.some((m) => m.id === catalog.active)) catalog.active = catalog.choices.find((m) => m.connection_id === selectedConnection)?.id || null;
  renderActiveModels();
}
$('model-preset').addEventListener('change', () => {
  const custom = $('model-preset').value === 'custom';
  $('custom-model-field').hidden = !custom;
  if (!custom) { $('connection-models').value = $('model-preset').value; updateModelChoices(); }
});
$('connection-models').addEventListener('change', updateModelChoices);
$('active-model').addEventListener('change', () => { settingsView.config.models.active = $('active-model').value || null; });
$('test-model').addEventListener('click', async () => {
  if (!api || busy || !settingsView) return;
  storeConnection(); const connection = currentConnection();
  const model = settingsView.config.models.choices.find((m) => m.connection_id === connection?.id);
  if (!model) { settingsNotice('请先填写模型名称。', true, 'test'); return; }
  setBusy(true); settingsNotice('正在验证连接与工具调用…', false, 'test');
  try {
    const report = await api.invoke('test_connection', { request:{ model:{ endpoint:connection.endpoint, api_key_env:connection.api_key_env, model:model.name }, key:pendingKeys.get(connection.id)?.key || null } });
    settingsNotice(`连接与工具调用通过，${report.elapsed_ms} ms。尚未保存；修改任何连接信息后需要重新测试。`, false, 'test');
  } catch (error) { settingsNotice(String(error), true, 'test'); }
  finally { setBusy(false); }
});
$('settings-form').addEventListener('submit', async (event) => {
  event.preventDefault(); if (!api || busy || !settingsView) return;
  storeConnection(); renderActiveModels();
  const config = settingsView.config;
  config.access_mode = $('access-mode').value; config.appearance = $('appearance').value;
  config.read_roots = $('read-roots').value.split('\n').map((x) => x.trim()).filter(Boolean);
  config.write_roots = $('write-roots').value.split('\n').map((x) => x.trim()).filter(Boolean);
  setBusy(true);
  try {
    settingsView = await api.invoke('save_settings', { update:{ expected_revision:settingsView.revision, config, keys:[...pendingKeys.values()] } });
    pendingKeys.clear(); $('connection-key').value = '';
    applyAppearance(settingsView.config.appearance); renderConnections();
    document.querySelectorAll('.message').forEach((node) => node.remove()); $('welcome').hidden = false; candidates([]); closeConfirmation();
    await init(); settingsNotice('设置已保存。新配置已生效，已开始新会话。', false, 'save');
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
