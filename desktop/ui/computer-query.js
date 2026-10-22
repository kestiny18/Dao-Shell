/* Cards are transient; workspace messages contain only visible text receipts. */
const computerTools = ['resource_snapshot','process_list','network_snapshot','app_list','computer_profile'];
const computerTitle = tool => typeof diagnosticTools!=='undefined'&&diagnosticTools.includes(tool) ? t(`diagnostics.tool.${tool}`) : t(['computer.resources','computer.processes','computer.network','computer.apps','computer.profile'][computerTools.indexOf(tool)] || 'computer.local');
const computerValue = v => v === null || v === undefined ? t('computer.unknown') : String(v);
const computerBytes = n => typeof n === 'number' ? `${(n / 1073741824).toFixed(2)} GiB` : t('computer.unknown');
const computerPercent = n => typeof n === 'number' ? `${n.toFixed(1)}%` : t('computer.unknown');
function computerLimits(value) { return t(value.kind === 'app_list' ? 'computer.limits.apps' : value.kind === 'network_snapshot' ? 'computer.limits.network' : value.kind === 'computer_profile' ? 'computer.limits.profile' : 'computer.limits.resources'); }
function computerStatus(code) { const key=`computer.status.${code}`; return Object.hasOwn(messages,key) ? t(key) : computerValue(code); }
function computerCoverageReason(reason) { const key=`computer.reason.${reason}`; return Object.hasOwn(messages,key) ? t(key) : String(reason); }
function observationLines(v) {
  if(typeof diagnosticTools!=='undefined'&&diagnosticTools.includes(v.kind))return diagnosticLines(v);
  const d = v.data || {}, lines = [];
  const row = (p,i) => `${i+1+(v.kind==='process_list' ? (v.page?.offset || 0) : 0)} ${p.name || ''} · ${computerBytes(p.memory_bytes)} · CPU ${computerPercent(p.cpu_percent_total)}`;
  if (v.kind === 'resource_snapshot') {
    const s = d.system || {};
    if ('cpu_percent' in s) lines.push(`CPU ${computerPercent(s.cpu_percent)}`);
    if ('memory_total_bytes' in s) lines.push(`${t('computer.memory')}: ${computerBytes(s.memory_used_bytes)} / ${computerBytes(s.memory_total_bytes)}`);
    for (const disk of s.disks || []) lines.push(`${disk.mount || ''} · ${computerBytes(disk.available_bytes)} / ${computerBytes(disk.total_bytes)}`);
    for (const sort of ['memory','cpu']) { if (d.process_rankings?.[sort]?.length) lines.push(`${sort.toUpperCase()} ↓`, ...d.process_rankings[sort].map(row)); }
  }
  if (v.kind === 'process_list') lines.push(...(d.processes || []).map(row));
  if (v.kind === 'app_list') lines.push(...(d.items || []).map((p,i) => `${i+1+(v.page?.offset || 0)} ${p.name || ''} · ${p.source||'registry'} · ${p.launch_status||''} · ${computerValue(p.version)} · ${computerValue(p.publisher)} · ${computerBytes(p.estimated_size_bytes)} · ${p.running === true ? t('computer.running') : t('computer.unknown')}`));
  if (v.kind === 'network_snapshot') lines.push(...(d.adapters || []).map(p => `${p.label} (${p.name || ''}) · IP ${computerValue(p.has_ip)} · ↓ ${typeof p.received_bytes_per_second === 'number' ? p.received_bytes_per_second.toFixed(0) : t('computer.unknown')} B/s · ↑ ${typeof p.transmitted_bytes_per_second === 'number' ? p.transmitted_bytes_per_second.toFixed(0) : t('computer.unknown')} B/s`));
  if (v.kind === 'computer_profile') lines.push(computerValue(d.os), ...(d.cpus || []), computerBytes(d.memory_bytes), ...(d.graphics || []), computerValue(d.architecture), `${computerValue(d.uptime_seconds)} s (${d.uptime_observed_at || ''})`);
  if(v.kind==='computer_profile'&&d.power)lines.push(`Power · AC ${computerValue(d.power.ac_online)} · Battery ${computerValue(d.power.battery_present)} · ${computerValue(d.power.percent)}% · ${computerValue(d.power.estimated_remaining_seconds)} s · ${d.power.reason||''}`);
  if (v.page) lines.push(`${v.page.returned} / ${v.page.matched}`);
  return lines;
}
function applyObservation(value, request) {
  if (!session(request.sessionId)) return;
  request.observations ||= new Set();
  const key = `${value.observation_id || value.error}:${JSON.stringify(value.page)}`;
  if (request.observations.has(key)) return;
  request.observations.add(key);
  const st = state(request.sessionId); st.observations ||= [];
  if (value.observation_id) {
    const index = st.observations.findIndex(v => v.observation_id === value.observation_id);
    if (index >= 0) st.observations.splice(index,1);
    st.observations.push(value); st.observations = st.observations.slice(-12);
    let seriesCount=0;st.observations=st.observations.reverse().filter(v=>v.kind!=='resource_observe'||++seriesCount<=4).reverse();
    while(st.observations.length&&new TextEncoder().encode(JSON.stringify(st.observations)).length>16*1024*1024)st.observations.shift();
    message(`${computerTitle(value.kind)} · ${localDate(value.observed_at)}\n${observationLines(value).join('\n')}\n${t('computer.scope')}\n${t('computer.history')}`,'receipt',false,request.sessionId);
  } else if (value.error) {
    st.observations.push(value);
  }
}
async function saveComputerSharing(mode) {
  if (busy || !api) return;
  setBusy(true);
  try {
    const view = await api.invoke('get_settings');
    view.config.computer_query_sharing = mode;
    await api.invoke('save_settings',{update:{expected_revision:view.revision,config:view.config,keys:[]}});
    settingsLoaded = false; invalidateContexts(); await init();
    persistenceNotice(t('computer.retry'));
  } catch (error) { persistenceNotice(errorText(error)); }
  finally { setBusy(false); }
}
async function ensureComputerSharing() {
  if (!api || busy) return false;
  const view = await api.invoke('get_settings');
  if (view.config.computer_query_sharing === 'enabled') return true;
  const id = session()?.id || await newSession();
  if (id) { state(id).observations.push({error:view.config.computer_query_sharing === 'disabled' ? 'permission_denied' : 'permission_required'}); renderCurrent(); }
  return false;
}
function renderObservations(values) {
  const target = $('computer-observations'); target.replaceChildren();
  for (const value of values.filter(v=>typeof diagnosticTools==='undefined'||!diagnosticTools.includes(v.kind))) {
    const card = document.createElement('details'); card.className = 'card computer-observation'; card.open = true;
    const title = document.createElement('summary'); title.textContent = value.error ? t(['permission_required','permission_denied'].includes(value.error) ? `computer.${value.error}` : 'computer.error') : `${computerTitle(value.kind)} · ${localDate(value.observed_at)}${value.cached ? ` · ${t('computer.cached')}` : ''}`;
    card.append(title);
    if (['permission_required','permission_denied'].includes(value.error)) {
      const note = document.createElement('p'); note.textContent = t('computer.privacy'); card.append(note);
      card.append(button(t('computer.allow'),t('computer.allow'),() => saveComputerSharing('enabled')),button(t('computer.deny'),t('computer.deny'),() => saveComputerSharing('disabled')));
    } else if (!value.error) {
      const body = document.createElement('div'); body.className = 'observation-lines'; body.textContent = observationLines(value).join('\n'); card.append(body);
      const note = document.createElement('p'); note.textContent = `${value.sample_window ? `${t('computer.window')}: ${value.sample_window.elapsed_ms} ms. ` : ''}${t('computer.scope')}\n${t('computer.coverage')}: ${computerStatus(value.status)} / ${computerStatus(value.coverage?.state)} ${(value.coverage?.reasons || []).map(reason => computerCoverageReason(reason)).join(', ')}\n${t('computer.limits')}: ${computerLimits(value)}`; card.append(note);
      const query = value.query || {tool:value.kind,arguments:{}};
      card.append(button(t('computer.refresh'),t('computer.refresh'),() => { const args = {...query.arguments}; delete args.cursor; delete args.observation_id; args.refresh = ['app_list','computer_profile'].includes(query.tool); if (!args.refresh) delete args.refresh; run('computer',JSON.stringify({tool:query.tool,arguments:args})); }));
      if (value.page?.next_cursor) card.append(button(t('computer.next'),t('computer.next'),() => { const args = {...query.arguments,cursor:value.page.next_cursor}; delete args.sample_ms; delete args.observation_id; delete args.refresh; run('computer',JSON.stringify({tool:query.tool,arguments:args})); }));
    }
    card.querySelectorAll('button').forEach(b => b.disabled = busy); target.append(card);
  }
}
$('local-file-search').addEventListener('click',() => { const input = $('input').value.trim(); if (input) run('search',input); });
$('local-computer-query').addEventListener('click',() => {
  const tool = $('local-computer-tool').value, args = {}, filter = $('local-computer-filter').value.trim();
  if (tool === 'process_list') {args.filter=filter;args.sort=$('local-computer-sort').value;}
  if (tool === 'app_list') {args.name=filter;args.view=$('local-computer-view').value;}
  run('computer',JSON.stringify({tool,arguments:args}));
});
function updateComputerControls() { const tool=$('local-computer-tool').value; $('local-computer-filter').hidden=!['process_list','app_list'].includes(tool); $('local-computer-sort').hidden=tool!=='process_list'; $('local-computer-view').hidden=tool!=='app_list'; }
$('local-computer-tool').addEventListener('change',updateComputerControls); updateComputerControls();
for (const key of ['computer.example_memory','computer.example_disk','computer.example_running']) {
  const example = button(t(key),t(key),() => { $('input').value=t(key); captureDraft(); saveSoon(); });
  example.dataset.i18n=key; document.querySelector('.suggestions').append(example);
}
