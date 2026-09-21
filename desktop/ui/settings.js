/* Settings drafts contain keys only until save/reload; saved keys never come back. */
let settingsView = null;
let selectedConnection = null;
let pendingKeys = new Map();
let settingsLoaded = false;

function settingsNotice(text, error = false) {
  $('settings-status').textContent = text;
  $('settings-status').hidden = !text;
  $('settings-status').classList.toggle('error', error);
}
function applyAppearance(value) { document.documentElement.dataset.theme = value; }
function selectOption(select, value, label) {
  const option = document.createElement('option'); option.value = value; option.textContent = label; select.append(option);
}
function currentConnection() { return settingsView?.config.models.connections.find((c) => c.id === selectedConnection); }
function storeConnection() {
  const connection = currentConnection();
  if (!connection) return;
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
    $('credential-state').textContent = settingsView.credential_available.includes(connection.id) ? '已有可用凭据 · 留空保留' : connection.api_key_env ? '尚未找到凭据' : '本机免密连接';
    $('provider-preset').value = 'custom';
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
  settingsView.config.models.connections.push({ id, label:'新连接', endpoint:'https://api.deepseek.com/chat/completions', api_key_env:`DAO_MODEL_${id.replaceAll('-', '_')}` });
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
  const preset = $('provider-preset').value;
  if (preset === 'deepseek') { $('connection-label').value = 'DeepSeek'; $('connection-endpoint').value = 'https://api.deepseek.com/chat/completions'; $('connection-models').value = 'deepseek-flash'; }
  if (preset === 'local') { $('connection-label').value = '本机模型'; $('connection-endpoint').value = 'http://127.0.0.1:11434/v1/chat/completions'; $('connection-models').value = ''; $('connection-env').value = ''; }
  $('connection-key').value = ''; pendingKeys.delete(selectedConnection);
  storeConnection(); renderActiveModels(); settingsNotice('预设已填入，请核对服务实际支持的模型名称。');
});
$('connection-models').addEventListener('change', () => { storeConnection(); renderActiveModels(); });
$('active-model').addEventListener('change', () => { settingsView.config.models.active = $('active-model').value || null; });
$('test-model').addEventListener('click', async () => {
  if (!api || busy || !settingsView) return;
  storeConnection(); const connection = currentConnection();
  const model = settingsView.config.models.choices.find((m) => m.connection_id === connection?.id);
  if (!model) { settingsNotice('请先填写模型名称。', true); return; }
  setBusy(true); settingsNotice('正在验证连接与工具调用…');
  try {
    const report = await api.invoke('test_connection', { request:{ model:{ endpoint:connection.endpoint, api_key_env:connection.api_key_env, model:model.name }, key:pendingKeys.get(connection.id)?.key || null } });
    settingsNotice(`连接与工具调用通过，${report.elapsed_ms} ms。尚未保存；修改任何连接信息后需要重新测试。`);
  } catch (error) { settingsNotice(String(error), true); }
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
    await init(); settingsNotice('设置已保存。新配置已生效，已开始新会话。');
  } catch (error) { settingsNotice(String(error), true); }
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
