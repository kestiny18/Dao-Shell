/* Settings drafts contain keys only until save/reload; saved keys never come back. */
let settingsView = null;
let selectedModel = null;
let pendingKeys = new Map();
let settingsLoaded = false;
let savedAccessMode = null;
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
  $('provider-help').textContent = preset ? t('settings.the_official_value_endpoint_is_filled_in_select_a_model_and_ente',{v0:(preset.label)}) : t('settings.connect_another_provider_or_local_model_enter_the_endpoint_and_m');
  $('model-preset').replaceChildren();
  for (const [value, label] of preset?.models || []) selectOption($('model-preset'), value, label);
  selectOption($('model-preset'), 'custom', t('settings.other_model'));
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
  $('active-model').replaceChildren(); selectOption($('active-model'), '', t('settings.do_not_use_a_model'));
  for (const model of catalog.choices) selectOption($('active-model'), model.id, model.name || t('settings.unnamed_model'));
  if (!catalog.choices.some(m => m.id === catalog.active)) catalog.active = null;
  $('active-model').value = catalog.active || '';
}
function renderModels() {
  $('model-list').replaceChildren();
  for (const model of settingsView.config.models.choices) selectOption($('model-list'), model.id, model.name || t('settings.unnamed_model'));
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
    $('credential-state').textContent = settingsView.credential_available.includes(connection.id) ? t('settings.credentials_available_leave_blank_to_keep') : t('settings.no_api_key_set_optional_for_keyless_local_models');
    $('provider-preset').value = providerFor(connection.endpoint); renderProvider();
  }
  renderActiveModels();
}
function renderDirectories() {
  for (const [id, field] of [['read-roots','read_roots'],['write-roots','write_roots']]) {
    const list = $(id); list.replaceChildren();
    settingsView.config[field].forEach((path,index) => {
      const row = document.createElement('li'), label = document.createElement('span'), remove = document.createElement('button');
      label.textContent = path; label.title = path; remove.type = 'button'; remove.textContent = t('settings.remove'); remove.setAttribute('aria-label', t('settings.remove_folder_value',{v0:(path)}));
      remove.addEventListener('click', () => { if (busy) return; settingsView.config[field].splice(index,1); renderDirectories(); });
      row.append(label,remove); list.append(row);
    });
    if (!list.children.length) { const row = document.createElement('li'); row.textContent = t('settings.no_folders_added'); list.append(row); }
  }
}
function directoryKey(path) { return path.replaceAll('\\','/').replace(/\/+$/, '').toLowerCase(); }
async function addDirectory(field) {
  if (!api || busy || !settingsView) return;
  setBusy(true);
  try {
    const path = await api.invoke('choose_directory');
    if (path && !settingsView.config[field].some(p => directoryKey(p) === directoryKey(path))) { settingsView.config[field].push(path); renderDirectories(); settingsNotice(t('settings.folder_added_to_draft_save_to_apply')); }
  } catch (error) { settingsNotice(t('settings.could_not_choose_a_folder_value',{v0:(errorText(error))}), true); }
  finally { setBusy(false); }
}
function accessDescription() {
  $('access-description').textContent = $('access-mode').value === 'full'
    ? t('settings.read_or_open_local_files_available_to_the_current_os_user_these_')
    : t('settings.these_folders_restrict_reading_opening_and_compatible_folder_sea');
}
async function loadSettings() {
  await languageReady;
  if (!api || busy) return;
  try {
    settingsView = await api.invoke('get_settings'); settingsLoaded = true; savedAccessMode=settingsView.config.access_mode;
    pendingKeys.clear(); selectedModel = settingsView.config.models.choices[0]?.id || null;
    $('access-mode').value = settingsView.config.access_mode;
    renderDirectories();
    $('appearance').value = settingsView.config.appearance;
    $('ui-language').value = settingsView.config.ui_language || 'auto';
    applyAppearance(settingsView.config.appearance);
    renderModels(); accessDescription(); settingsNotice('');
  } catch (error) { settingsNotice(errorText(error), true); }
}
$('model-list').addEventListener('change', () => { storeModel(); selectedModel = $('model-list').value; renderModels(); });
$('add-model').addEventListener('click', () => {
  if (!settingsView || busy) return;
  storeModel(); const id = crypto.randomUUID(), modelId = crypto.randomUUID();
  settingsView.config.models.connections.push({id,label:t('settings.model_configuration'),endpoint:providerPresets.deepseek.endpoint,api_key_env:`DAO_MODEL_${id.replaceAll('-', '_')}`});
  settingsView.config.models.choices.push({id:modelId,connection_id:id,name:'deepseek-flash'});
  selectedModel = modelId; renderModels(); settingsNotice(t('settings.enter_the_model_details_test_then_save_choose_the_default_model_'));
});
$('remove-model').addEventListener('click', () => {
  if (!settingsView || busy) return;
  const catalog = settingsView.config.models, connectionId = currentModel()?.connection_id;
  catalog.choices = catalog.choices.filter(m => m.id !== selectedModel);
  if (!catalog.choices.some(m => m.connection_id === connectionId)) catalog.connections = catalog.connections.filter(c => c.id !== connectionId);
  pendingKeys.delete(selectedModel); selectedModel = catalog.choices[0]?.id || null;
  renderModels(); settingsNotice(t('settings.the_model_will_be_removed_when_saved_stored_os_credentials_will_'));
});
$('provider-preset').addEventListener('change', () => {
  if (!currentModel()) return;
  const preset = providerPresets[$('provider-preset').value];
  $('connection-endpoint').value = preset?.endpoint || '';
  $('connection-models').value = preset?.models[0][0] || '';
  $('connection-env').value = ''; $('connection-key').value = ''; pendingKeys.delete(selectedModel);
  storeModel(); renderModels(); settingsNotice(t('settings.provider_changed_enter_its_model_details_then_save_to_apply'));
});
function updateModelChoices() {
  storeModel(); renderActiveModels();
  const option = [...$('model-list').options].find(o => o.value === selectedModel);
  if (option) option.textContent = currentModel()?.name || t('settings.unnamed_model');
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
  if (!model) { settingsNotice(t('settings.enter_a_model_id_first'), true, 'test'); return; }
  setBusy(true); settingsNotice(t('settings.checking_connection_and_tool_calls'), false, 'test');
  try {
    const report = await api.invoke('test_connection', { request:{ model:{ endpoint:connection.endpoint, api_key_env:connection.api_key_env, model:model.name }, key:pendingKeys.get(model.id)?.key || null } });
    settingsNotice(t('settings.connection_and_tool_calls_passed_in_value_ms_not_saved_yet_test_',{v0:(report.elapsed_ms)}), false, 'test');
  } catch (error) { settingsNotice(errorText(error), true, 'test'); }
  finally { setBusy(false); }
});
$('settings-form').addEventListener('submit', async (event) => {
  event.preventDefault(); if (!api || busy || !settingsView) return;
  storeModel(); renderActiveModels();
  const config = settingsView.config;
  const newlyFull=savedAccessMode!=='full' && $('access-mode').value==='full';
  config.access_mode = $('access-mode').value; config.appearance = $('appearance').value; config.ui_language=$('ui-language').value;
  setBusy(true);
  try {
    settingsView = await api.invoke('save_settings', { update:{ expected_revision:settingsView.revision, config, keys:[...pendingKeys.values()] } });
    pendingKeys.clear(); $('connection-key').value = '';
    applyLanguage(settingsView.effective_language);
    applyAppearance(settingsView.config.appearance); renderModels();
    invalidateContexts();
    savedAccessMode=settingsView.config.access_mode;
    await init(); settingsNotice(t('settings.settings_saved_history_is_preserved_model_context_and_old_candid'), false, 'save');
    if(newlyFull) {
      const component=await api.invoke('index_service_command',{action:'status',confirmed:false,deleteIndex:false});
      const action={not_installed:'install',version_mismatch:'upgrade',stopped:'service-start'}[component.state];
      if(action && component.payload_ready) await indexServiceAction(action);
      if(component.state!=='running') settingsNotice(t('settings.full_access_saved_whole_volume_search_needs_an_available_matchin'),false,'save');
    }
  } catch (error) { settingsNotice(errorText(error), true, 'save'); }
  finally { setBusy(false); }
});
$('reload-settings').addEventListener('click', loadSettings);
$('access-mode').addEventListener('change', accessDescription);
document.querySelectorAll('[data-settings]').forEach((button) => button.addEventListener('click', () => {
  for (const section of ['models','access','inventory','general']) $(`settings-${section}`).hidden = section !== button.dataset.settings;
  document.querySelectorAll('[data-settings]').forEach((other) => other.setAttribute('aria-pressed', String(other === button)));
}));
document.addEventListener('pagechange', (event) => { if (event.detail === 'settings' && !settingsLoaded) loadSettings(); });
loadSettings();

let inventoryLoading = false;
async function inventoryAction(action, id = null, path = null) {
  if (!api || inventoryLoading) return;
  inventoryLoading = true;
  try {
    const rows = await api.invoke('inventory_command', {action,id,path});
    $('inventory-list').replaceChildren();
    const labels = {building:t('settings.recording'),ready:t('settings.available'),pending:t('settings.update_pending'),damaged:t('settings.cache_damaged'),unavailable:t('settings.folder_unavailable')};
    for (const item of rows) {
      const row = document.createElement('li');
      const detail = document.createElement('div');
      detail.textContent = t('settings.value_value_value_entriesvalue_value',{v0:(item.root==='（当前无访问权限）' ? localBuiltin(item.root) : item.root),v1:(builtinText(labels[item.state]) || item.state),v2:(item.entries),v3:(item.updated_at ? ' · ' + localDate(item.updated_at) : ''),v4:(localBuiltin(item.message))});
      row.append(detail);
      for (const [operation,label] of [['update',t('settings.update_now')],['stop',t('settings.stop_this_update')],['remove',t('settings.remove')]]) {
        if (operation === 'stop' && item.state !== 'building') continue;
        const button = document.createElement('button');button.type='button';button.textContent=label;
        button.disabled = operation === 'update' && (item.state === 'building' || item.state === 'unavailable');
        button.addEventListener('click',()=>inventoryAction(operation,item.id));row.append(button);
      }
      $('inventory-list').append(row);
    }
    $('inventory-status').textContent = rows.length ? '' : t('settings.no_accelerated_folders_added');
  } catch (error) { $('inventory-status').textContent=errorText(error); }
  finally { inventoryLoading=false; }
}
$('add-inventory').addEventListener('click',async()=> {
  if (!api || inventoryLoading) return;
  try { const path=await api.invoke('choose_directory');if(path) await inventoryAction('add',null,path); }
  catch(error) { $('inventory-status').textContent=errorText(error); }
});
document.querySelector('[data-settings="inventory"]').addEventListener('click',()=>inventoryAction('status'));
setInterval(()=>{if(!$('settings-inventory').hidden && !$('page-settings').hidden) inventoryAction('status');},1500);

let indexServiceLoading = false;
const indexLabels = {accepted:t('settings.rebuild_accepted_check_volume_status_for_completion'),in_progress:t('settings.rebuild_is_already_in_progress'),rate_limited:t('settings.wait_before_requesting_another_rebuild'),rebuild_queued:t('settings.rebuild_queued'),rebuilding:t('settings.rebuilding'),not_installed:t('settings.not_installed'),running:t('settings.service_running'),stopped:t('settings.service_stopped'),unavailable:t('settings.service_unavailable_or_identity_check_failed'),version_mismatch:t('settings.service_version_mismatch_update_required'),payload_missing:t('settings.installation_payload_not_prepared'),unsupported_platform:t('settings.this_operating_system_is_not_supported'),building:t('settings.recording_for_the_first_time'),catching_up:t('settings.catching_up_with_changes'),ready:t('settings.published'),coverage_incomplete:t('settings.incomplete_coverage'),retrying:t('settings.files_are_changing_retrying'),rebuild_required:t('settings.index_rebuild_required'),error:t('settings.update_failed'),timeout:t('settings.query_timed_out'),initializing:t('settings.initializing'),uac_cancelled:t('settings.administrator_authorization_canceled'),completed:t('settings.operation_completed'),failed:t('settings.operation_failed_check_service_status'),outcome_unknown:t('settings.outcome_unknown_refresh_status_do_not_repeat_the_same_operation_')};
const indexReasons = {offline:t('settings.volume_offline'),stopping_offline:t('settings.volume_offline_stopping_the_old_index'),discovery_failed:t('settings.volume_discovery_failed_keeping_the_previous_catalog'),access_denied:t('settings.some_system_metadata_is_inaccessible'),sharing_violation:t('settings.some_files_are_temporarily_in_use'),delete_pending:t('settings.some_files_are_being_deleted'),file_metadata_not_supported:t('settings.some_metadata_is_unsupported'),unresolved_parent_chain:t('settings.some_parent_relationships_are_unresolved'),metadata_unavailable:t('settings.some_metadata_is_unavailable'),coverage_incomplete:t('settings.index_coverage_is_incomplete'),query_unavailable:t('settings.query_unavailable'),query_timeout:t('settings.query_reached_its_time_budget'),journal_unavailable:t('settings.volume_change_journal_unavailable'),worker_unavailable:t('settings.volume_index_unavailable'),worker_panicked:t('settings.volume_index_error'),worker_stopped:t('settings.volume_index_stopped'),volume_worker_limit:t('settings.volume_count_limit_exceeded'),unsupported_filesystem:t('settings.file_system_unsupported'),not_fixed_volume:t('settings.not_a_fixed_local_volume'),volume_unavailable:t('settings.volume_unavailable'),mount_or_candidate_changed:t('settings.volume_mapping_or_file_identity_changed'),candidate_changed:t('settings.candidate_changed_during_validation'),no_supported_volume:t('settings.no_supported_volume')};
function indexReason(value) { return builtinText(indexReasons[value]) || t('settings.some_records_are_unavailable'); }
Object.assign(indexReasons, {
  maintenance_stopped:t('settings.maintenance_stopped_stop_and_restart_the_service_under_service_m'),
  storage_identity_changed:t('settings.index_storage_identity_changed_check_storage_and_repair_the_serv'),
  rebuild_required:t('settings.index_rebuild_required'),
  identity_changed:t('settings.volume_identity_changed_refresh_status'),
  metadata_changed_retrying:t('settings.metadata_is_changing_limited_retries_in_progress'),
  metadata_changed_retry_exhausted:t('settings.metadata_keeps_changing_try_a_manual_rebuild_later'),
  request_failed:t('settings.index_update_failed_try_a_manual_rebuild_and_check_the_service_i'),
  budget_exhausted:t('settings.index_update_reached_its_budget_check_the_service_and_retry'),
});
function indexVolumeText(volume) {
  const status=volume.index_status || volume; const location=volume.volume || volume.mounts?.[0] || t('settings.unmounted_volume');
  const phase=status.phase || (volume.worker_started ? 'initializing' : 'unavailable');
  const reasons=[...new Set([...(status.reasons || status.coverage_reasons || []),status.error,volume.worker_error,volume.reason].filter(Boolean))];
  const updated=status.updated_unix_ms ? localDate(Number(status.updated_unix_ms)) : t('settings.not_published_yet');
  const restart=!volume.worker_started && ['worker_unavailable','worker_panicked','worker_stopped','identity_changed','rebuild_required'].includes(volume.worker_error);
  return `${location} · ${builtinText(indexLabels[phase]) || t('settings.unknown_status')}${status.generation == null ? '' : t('settings.generation_value',{v0:(status.generation)})} · ${updated}${reasons.length ? ` · ${reasons.map(indexReason).join('；')}` : ''}${restart ? t('settings.stop_and_restart_the_service_under_service_management_then_check') : ''}`;
}
async function indexServiceAction(action='status',volumeKey=null) {
  if (!api || indexServiceLoading) return;
  let confirmed=false;const deleteIndex=action==='uninstall' && $('index-delete-data').checked;
  if (!['status','diagnostic'].includes(action)) {
    confirmed=await ask(action==='rebuild' ? t('settings.record_all_file_names_on_this_volume_again_the_old_generation_re') : t('settings.confirm_value_value_only_this_explicit_action_requests_administr',{v0:(action==='install' ? t('settings.install_the_whole_volume_name_index_component_administrator_perm') : action==='upgrade' ? t('settings.update_the_whole_volume_name_index_component_administrator_permi') : t('settings.perform_this_service_management_action')),v1:(deleteIndex ? t('settings.index_records_will_also_be_deleted_source_files_are_unaffected') : t('settings.index_records_are_preserved_by_default'))}));
    if(!confirmed) return;
  }
  indexServiceLoading=true;
  const buttons=[$('index-refresh'),$('index-diagnostic'),...document.querySelectorAll('[data-index-action]')];buttons.forEach(button=>button.disabled=true);
  try {
    const value=await api.invoke('index_service_command',{action,confirmed,deleteIndex,volumeKey});
    if(action==='diagnostic') { $('index-diagnostic-text').value=JSON.stringify(value,null,2);$('index-report').hidden=false;$('index-report').open=true; }
    if(!['status','diagnostic'].includes(action)) $('index-service-operation').textContent=builtinText(indexLabels[value.state]) || t('settings.operation_returned_refresh_status');
    const current=action==='status' ? value : await api.invoke('index_service_command',{action:'status',confirmed:false,deleteIndex:false});
    const guidance={stopped:t('settings.start_the_index_component_under_service_management'),version_mismatch:t('settings.update_the_service_under_service_management_then_refresh_status'),unavailable:t('settings.refresh_status_if_still_unavailable_check_or_repair_the_service_')};
    $('index-service-status').textContent=`${current.enabled ? t('settings.full_access_whole_volume_name_search_by_default') : t('settings.restricted_access_authorized_folders_only')} · ${builtinText(indexLabels[current.state]) || t('settings.unknown_status')}${guidance[current.state] ? ` · ${guidance[current.state]}` : ''}${current.payload_ready ? '' : t('settings.installation_payload_missing_this_build_cannot_install_the_servi')}`;
    $('index-service-volumes').replaceChildren();
    for(const volume of current.volumes || []) {const row=document.createElement('li');row.textContent=indexVolumeText(volume);const writerStopped=['maintenance_stopped','storage_identity_changed'].includes(volume.index_status?.error);if(current.state==='running' && volume.supported && volume.worker_started && !volume.worker_error && !writerStopped && volume.pipe?.startsWith('volume-')) {const button=document.createElement('button');button.textContent=t('settings.rebuild_index');button.addEventListener('click',()=>indexServiceAction('rebuild',volume.pipe.slice(7)));row.append(button);}$('index-service-volumes').append(row);}
    buttons.forEach(button=>button.disabled=Boolean(current.busy) || (!current.payload_ready && button!==$('index-refresh') && button!==$('index-diagnostic')));
  } catch(error) {$('index-service-status').textContent=errorText(error);buttons.forEach(button=>button.disabled=false);}
  finally {indexServiceLoading=false;}
}
$('index-diagnostic').addEventListener('click',()=>indexServiceAction('diagnostic'));
$('index-copy').addEventListener('click',async()=>{
  const text=$('index-diagnostic-text');
  try { await navigator.clipboard.writeText(text.value);$('index-copy-status').textContent=t('settings.report_copied'); }
  catch {text.focus();text.select();$('index-copy-status').textContent=t('settings.press_ctrl_c_to_copy_the_selected_report');}
});
$('index-refresh').addEventListener('click',()=>indexServiceAction('status'));
document.querySelectorAll('[data-index-action]').forEach(button=>button.addEventListener('click',()=>indexServiceAction(button.dataset.indexAction)));
document.querySelector('[data-settings="inventory"]').addEventListener('click',()=>indexServiceAction());
setInterval(()=>{if(!$('settings-inventory').hidden&&!$('page-settings').hidden) indexServiceAction();},5000);

document.addEventListener('languagechange',()=>{if(!settingsLoaded || !settingsView)return;renderModels();renderDirectories();accessDescription();});
