let homeSnapshot = null;
function resourcePercent(value) { return value == null || !Number.isFinite(value) ? t('home.unknown') : value > 0 && value < 0.1 ? '<0.1%' : `${value.toFixed(1)}%`; }
function terminationButton(item) {
  const button=textNode('button',item.termination_ref ? t('home.end') : t('home.unavailable'),'process-end');button.type='button';
  button.dataset.terminationRef=item.termination_ref || '';
  button.disabled=busy || !item.termination_ref;
  button.title=localBuiltin(item.termination_reason) || t('home.end_confirmed_process_instances_after_local_confirmation');
  button.setAttribute('aria-label',`${item.name || t('home.process')}：${button.textContent}。${button.title}`);
  button.addEventListener('click',event=>{event.stopPropagation();if(item.termination_ref) terminateSelection(item.termination_ref);});
  button.addEventListener('dblclick',event=>event.stopPropagation());
  return button;
}
async function terminateSelection(reference) {
  if(!api || busy) return;
  closeAppMenu();applicationBusy=true;setBusy(true);
  const status=text=>{ $('process-action-status').textContent=text;$('app-process-action-status').textContent=text; };
  let confirmed=false;
  try {
    const prepared=await api.invoke('process_action',{action:'prepare',reference,confirmed:false});
    const list=(prepared.processes || []).map(p=>`${p.name} / PID ${p.pid}`).join('；');
    const answer=ask(t('home.end_these_value_confirmed_processes_value_value',{v0:(prepared.count),v1:(list),v2:(localBuiltin(prepared.warning))}));
    const pendingDecision=decision;const deadline=setTimeout(()=>{if(decision===pendingDecision) decide(false);},90000);
    try {confirmed=await answer;} finally {clearTimeout(deadline);}
    const reply=await api.invoke('process_action',{action:'resolve',reference:prepared.confirmation_id,confirmed});
    status(processOutcome(reply) || t('home.refresh_the_overview_to_check_process_status'));
  } catch(error) {status(errorText(error));}
  finally {applicationBusy=false;setBusy(false);if(confirmed) await refreshOverview();}
}
function renderProcesses() {
  const snapshot = homeSnapshot;
  let count = 0;
  for (const ranking of ['cpu', 'memory']) {
    const processes = snapshot?.process_rankings?.[ranking] || [];
    const body = $(`resource-${ranking}-processes`);
    body.replaceChildren();
    for (const process of processes) {
      const row = textNode('tr', '');
      row.append(textNode('td', `${process.name || t('home.unknown_name')} / ${process.pid}`), textNode('td', resourcePercent(process.cpu_percent_total)), textNode('td', process.memory_bytes == null ? t('home.unknown') : humanBytes(process.memory_bytes)));
      const action=textNode('td','');action.append(terminationButton(process));row.append(action);
      body.append(row);
    }
    $(`resource-${ranking}-empty`).hidden = processes.length > 0;
    count += processes.length;
  }
  $('resource-empty').hidden = count > 0;
  $('resource-sample').textContent = snapshot ? t('home.observed_value_cpu_window_value_manual_refresh_only_no_trend_his',{v0:(localDate(snapshot.observed_at)),v1:(Number.isFinite(snapshot.cpu_interval_ms) ? (snapshot.cpu_interval_ms / 1000).toFixed(2) + t('home.seconds') : t('home.unknown'))}) : t('home.waiting_for_sample');
  $('resource-coverage').textContent = t('home.enumerated_value_processes_showing_the_top_value_cpu_and_top_val',{v0:(snapshot?.matched_processes ?? t('home.unknown_173')),v1:(snapshot?.process_rankings?.cpu?.length ?? 0),v2:(snapshot?.process_rankings?.memory?.length ?? 0),v3:(snapshot?.disappeared_processes ?? t('home.unknown_173'))});
}
let appMenu = null;
function closeAppMenu() { appMenu?.remove(); appMenu = null; }
async function applicationAction(app, action) {
  closeAppMenu();
  if (!api || busy || !app.action_id) return;
  if (action !== 'uninstall' && app.action_reason) { $('activity').textContent = localBuiltin(app.action_reason); return; }
  applicationBusy = true; setBusy(true); $('activity').textContent = action === 'admin' ? t('home.waiting_for_windows_administrator_confirmation') : t('home.requesting_windows');
  try { const reply = await api.invoke('application_action', {appId:app.action_id, action}); $('activity').textContent = localBuiltin(reply.message); }
  catch (error) { $('activity').textContent = errorText(error); }
  finally { applicationBusy = false; setBusy(false); }
}
function showAppMenu(app, row, event) {
  event.preventDefault(); closeAppMenu();
  const menu = textNode('div', '', 'app-context-menu'); appMenu = menu;
  menu.setAttribute('role', 'menu'); menu.setAttribute('aria-label', t('home.actions_for_value',{v0:(app.name)}));
  for (const [action, label] of [['run',t('home.run')],['admin',t('home.run_as_administrator')],['location',t('home.open_file_location')],['uninstall',t('home.uninstall_open_windows_settings')]]) {
    const button = textNode('button', label); button.type = 'button'; button.setAttribute('role','menuitem');
    const reason = busy ? t('home.wait_for_the_current_operation_to_finish') : !app.action_id ? t('home.refresh_the_overview_to_obtain_action_references') : action !== 'uninstall' ? localBuiltin(app.action_reason) : '';
    button.setAttribute('aria-disabled', String(Boolean(reason)));
    if (reason) { button.title = reason; button.append(textNode('small', reason)); }
    button.addEventListener('click', () => { if (!reason) applicationAction(app, action); });
    menu.append(button);
  }
  document.body.append(menu);
  const rect = row.getBoundingClientRect();
  menu.style.left = `${Math.max(0, Math.min(event.clientX || rect.left, window.innerWidth - menu.offsetWidth))}px`;
  menu.style.top = `${Math.max(0, Math.min(event.clientY || rect.bottom, window.innerHeight - menu.offsetHeight))}px`;
  menu.firstElementChild.focus();
  menu.addEventListener('keydown', e => {
    const buttons = [...menu.querySelectorAll('button')]; const i = buttons.indexOf(document.activeElement);
    if (e.key === 'Escape' || e.key === 'Tab') { closeAppMenu(); row.focus(); if (e.key === 'Escape') e.preventDefault(); }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); buttons[(i + (e.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length].focus(); }
  });
}
document.addEventListener('pointerdown', e => { if (appMenu && !appMenu.contains(e.target)) closeAppMenu(); });
window.addEventListener('resize', closeAppMenu);
document.addEventListener('pagechange', closeAppMenu);
document.addEventListener('scroll', closeAppMenu, true);
let appView = readPreference('dao.appView', 'cards') === 'list' ? 'list' : 'cards';
function humanBytes(value) {
  if (value == null || !Number.isFinite(value)) return t('home.unavailable_183');
  const units = ['B','KiB','MiB','GiB','TiB']; let index = 0;
  while (value >= 1024 && index < units.length - 1) { value /= 1024; index++; }
  return `${value.toFixed(index ? 1 : 0)} ${units[index]}`;
}
function textNode(tag, text, className) { const node = document.createElement(tag); node.textContent = text; if (className) node.className = className; return node; }
function renderApps() {
  closeAppMenu();
  const query = $('app-filter').value.trim().toLowerCase();
  const matches = (homeSnapshot?.applications.items || []).filter((item) => item.name.toLowerCase().includes(query))
    .sort((a, b) => Number(b.running === true) - Number(a.running === true));
  $('applications').replaceChildren();
  $('applications').dataset.view = appView;
  document.querySelectorAll('[data-app-view]').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.appView === appView)));
  for (const app of matches) {
    const row = textNode('article', '', 'app-item');
    row.tabIndex = 0; row.setAttribute('aria-label', t('home.value_right_click_or_press_shift_f10_for_actions',{v0:(app.name)}));
    row.addEventListener('dblclick', () => applicationAction(app, 'run'));
    row.addEventListener('contextmenu', e => showAppMenu(app, row, e));
    row.addEventListener('keydown', e => { if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) showAppMenu(app, row, e); });
    const initials = Array.from(app.name.trim()).slice(0, 2).join('').toUpperCase();
    const icon = textNode('div', initials, 'app-icon'); icon.setAttribute('aria-hidden', 'true');
    if (typeof app.icon === 'string' && app.icon.length < 24000 && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(app.icon)) {
      const image = document.createElement('img'); image.alt = ''; image.src = app.icon;
      image.addEventListener('error', () => icon.replaceChildren(document.createTextNode(initials)), { once:true });
      icon.replaceChildren(image);
    }
    const info = textNode('div', '', 'app-info');
    info.append(textNode('strong', app.name), textNode('small', [app.version, app.publisher].filter(Boolean).join(' · ') || t('home.version_information_not_registered')));
    info.append(textNode('small', app.running === true ? t('home.running') : t('home.running_state_unknown'), app.running === true ? 'running-badge' : ''));
    info.append(textNode('small', t('home.estimated_installed_size_value',{v0:(app.estimated_size_bytes == null ? t('home.unknown') : humanBytes(app.estimated_size_bytes))})));
    row.append(icon, info);
    if(app.running===true) {const actions=textNode('div','','app-process-controls');actions.append(terminationButton(app));if(app.running_instances) actions.append(textNode('small',t('home.only_value_confirmed_instances',{v0:(app.running_instances)})));if(!app.termination_ref) actions.append(textNode('small',localBuiltin(app.termination_reason) || t('home.no_reliable_process_association_cannot_end_safely')));row.append(actions);}
    $('applications').append(row);
  }
  if (!matches.length) $('applications').append(textNode('p', t('home.no_matching_application_records_in_this_scope')));
}
function renderOverview(snapshot) {
  homeSnapshot = snapshot; const system = snapshot.system;
  $('device-summary').textContent = [snapshot.device.host, snapshot.device.os].filter(Boolean).join(' · ');
  $('cpu-value').textContent = Number.isFinite(system.cpu_percent) ? `${system.cpu_percent.toFixed(1)}%` : t('home.unavailable_183');
  const usedMemory = system.memory_used_bytes, totalMemory = system.memory_total_bytes;
  const memoryKnown = Number.isFinite(usedMemory) && usedMemory >= 0 && Number.isFinite(totalMemory) && totalMemory > 0 && usedMemory <= totalMemory;
  $('memory-value').textContent = memoryKnown ? `${Math.round(100 * usedMemory / totalMemory)}%` : t('home.unavailable_183');
  $('memory-detail').textContent = t('home.used_value_total_value_available_value',{v0:(humanBytes(usedMemory)),v1:(humanBytes(totalMemory)),v2:(humanBytes(system.memory_available_bytes))});
  $('uptime-value').textContent = t('home.value_hours',{v0:(Math.floor(snapshot.device.uptime_seconds / 3600))});
  $('architecture').textContent = t('home.value_value_logical_processors',{v0:(snapshot.device.architecture || t('home.unknown_architecture')),v1:(snapshot.logical_cpus)});
  $('observed-at').textContent = t('home.observed_at_value',{v0:(localDate(snapshot.observed_at,true))});
  $('disks').replaceChildren();
  for (const disk of system.disks || []) {
    const card = textNode('article', '', 'card disk');
    const used = Number.isFinite(disk.total_bytes) && disk.total_bytes > 0 && Number.isFinite(disk.available_bytes) && disk.available_bytes >= 0 && disk.available_bytes <= disk.total_bytes ? 100 * (1 - disk.available_bytes / disk.total_bytes) : null;
    const meter = document.createElement('progress'); meter.max = 100; if (used != null) meter.value = used;
    meter.setAttribute('aria-label', t('home.used_space_on_value',{v0:(pathText(disk.mount))}));
    meter.setAttribute('aria-valuetext', used == null ? t('home.space_usage_unknown') : `${Math.round(used)}%`);
    card.append(textNode('strong', pathText(disk.mount)), textNode('p', t('home.available_value_value',{v0:(humanBytes(disk.available_bytes)),v1:(humanBytes(disk.total_bytes))})), meter);
    $('disks').append(card);
  }
  if (!system.disks?.length) $('disks').append(textNode('p', t('home.no_disk_capacity_data_available_in_this_sample')));
  $('networks').replaceChildren(); $('network-count').textContent = t('home.value',{v0:(snapshot.network.adapters.length)});
  $('network-note').textContent = localBuiltin(snapshot.network.limits);
  for (const adapter of snapshot.network.adapters) {
    const row = textNode('article', '', 'network-card');
    row.append(textNode('strong', adapter.name), textNode('small', `${adapter.has_ip ? t('home.ip_configured') : t('home.no_ip_observed')} · ↓ ${humanBytes(adapter.received_bytes_per_second)}/s · ↑ ${humanBytes(adapter.transmitted_bytes_per_second)}/s`));
    $('networks').append(row);
  }
  $('app-count').textContent = snapshot.applications.available ? t('home.value_records',{v0:(snapshot.applications.items.length)}) : t('home.unavailable_205');
  const apps = snapshot.applications;
  $('apps-value').textContent = apps.available ? t('home.value_records_206',{v0:(apps.items.length)}) : t('home.unavailable_183');
  $('apps-detail').textContent = apps.available ? t('home.value_known_running_apps_unknown_does_not_mean_stopped',{v0:(apps.items.filter(app => app.running === true).length)}) : t('home.registration_and_running_state_not_yet_confirmed');
  $('app-note').textContent = localBuiltin(snapshot.applications.limits) + (snapshot.applications.observed_at ? t('home.applications_observed_at_value',{v0:(localDate(snapshot.applications.observed_at,true))}) : '') + (snapshot.applications.unreadable_process_paths ? t('home.value_process_paths_unreadable_possibly_due_to_os_permissions',{v0:(snapshot.applications.unreadable_process_paths)}) : '') + (snapshot.applications.partial ? t('home.some_registry_locations_were_unreadable_or_the_enumeration_limit') : '');
  renderProcesses(); renderApps(); $('explain-home').disabled = busy;
}
async function refreshOverview() {
  await languageReady;
  if (!api || busy) return;
  closeAppMenu();
  setBusy(true); $('activity').textContent = t('home.reading_this_computer_s_overview');
  try { renderOverview(await api.invoke('computer_overview')); $('activity').textContent = t('home.computer_overview_updated'); }
  catch (error) { $('activity').textContent = errorText(error); $('device-summary').textContent = homeSnapshot ? t('home.refresh_failed_keeping_the_previous_dated_sample_below') : t('home.overview_unavailable_refresh_again_later'); }
  finally { setBusy(false); $('explain-home').disabled = !homeSnapshot; }
}
$('refresh-home').addEventListener('click', refreshOverview);
document.querySelectorAll('[data-overview-target]').forEach(button => {
  const details = $(button.dataset.overviewTarget);
  details.addEventListener('toggle', () => button.setAttribute('aria-expanded', String(details.open)));
  button.addEventListener('click', () => {
    details.open = true;
    button.setAttribute('aria-expanded', 'true');
    details.querySelector('summary').focus({preventScroll:true});
    details.scrollIntoView({behavior:window.matchMedia?.('(prefers-reduced-motion: reduce)')?.matches ? 'auto' : 'smooth',block:'start'});
  });
});
$('app-filter').addEventListener('input', renderApps);
document.querySelectorAll('[data-app-view]').forEach((button) => button.addEventListener('click', () => {
  appView = button.dataset.appView; savePreference('dao.appView', appView); renderApps();
}));
$('explain-home').addEventListener('click', () => { if (homeSnapshot) run('explain'); });
refreshOverview();

// Keep the profile out of overview/model facts and persistence. Only this whitelist
// becomes visible and copyable; IPC objects may contain additional fields.
let profileSnapshot = null;
let profileText = '';
function profileSummary(profile) {
  const label = value => typeof value === 'string' && value.trim() ? value : t('home.unknown');
  const names = values => Array.isArray(values) && values.length ? values.map(label).join('；') : t('home.unknown');
  const size = value => Number.isFinite(value) && value > 0 ? humanBytes(value) : t('home.unknown');
  const volumes = Array.isArray(profile.volumes) && profile.volumes.length ? profile.volumes.map((bytes, i) => t('home.volume_value_value',{v0:(i + 1),v1:(size(bytes))})).join('；') : t('home.unknown');
  return [t('home.brand_value',{v0:(label(profile.brand))}), t('home.model_value',{v0:(label(profile.model))}), t('home.operating_system_value',{v0:(label(profile.os))}), `CPU：${names(profile.cpus)}`, t('home.total_memory_value',{v0:(size(profile.memory_bytes))}), t('home.display_adapters_value',{v0:(names(profile.graphics))}), t('home.volume_capacities_not_physical_disks_do_not_add_value',{v0:(volumes)}), t('home.observed_at_value_223',{v0:(label(profile.observed_at))})].join('\n');
}
async function refreshProfile() {
  if (!api || busy) { $('profile-status').textContent = t('home.wait_for_the_current_operation_to_finish_before_reading_the_prof'); return; }
  setBusy(true); $('profile-status').textContent = t('home.reading_this_computer_s_profile');
  try {
    const profile = await api.invoke('computer_profile');
    profileText = profileSummary(profile); profileSnapshot = profile;
    $('profile-summary').textContent = profileText;
    $('profile-status').textContent = t('home.read_complete_copy_includes_only_the_summary_below_without_host_');
  } catch (error) { $('profile-status').textContent = t('home.read_failed_valuevalue',{v0:(errorText(error)),v1:(profileSnapshot ? t('home.keeping_the_previous_summary_and_observation_time') : '')}); }
  finally { setBusy(false); }
}
$('computer-profile').addEventListener('toggle', () => { if ($('computer-profile').open && !profileSnapshot) refreshProfile(); });
$('refresh-profile').addEventListener('click', refreshProfile);
$('copy-profile').addEventListener('click', async () => {
  if (!profileText) { $('profile-status').textContent = t('home.read_the_computer_profile_first'); return; }
  try {
    if (!navigator.clipboard?.writeText) throw Error(t('home.clipboard_unavailable_select_and_copy_the_summary_below_manually'));
    await navigator.clipboard.writeText(profileText);
    $('profile-status').textContent = t('home.summary_copied');
  } catch (error) { $('profile-status').textContent = t('home.copy_failed_value',{v0:(errorText(error))}); }
});

document.addEventListener('languagechange',()=>{if(homeSnapshot)renderOverview(homeSnapshot);if(profileSnapshot){profileText=profileSummary(profileSnapshot);$('profile-summary').textContent=profileText;}});

function processOutcome(reply) {
  if(uiLanguage==='zh-CN' || !Array.isArray(reply.processes) || !reply.processes.length)return localBuiltin(reply.message);
  const codes={terminated:'已退出',already_exited:'原进程已退出',pending:'已请求结束，仍在等待退出',identity_changed:'身份已变化，未操作',cancelled:'已取消，未操作',access_denied:'Windows 拒绝访问，未报告退出',protected_process:'受保护进程，未操作',other_user:'非当前用户会话，未操作',other_session:'非当前用户会话，未操作'};
  const ended=reply.processes.filter(p=>['terminated','already_exited'].includes(p.state)).length;
  const pending=reply.processes.filter(p=>p.state==='pending').length;
  return t('process.outcome',{ended,total:reply.processes.length,pending,detail:reply.processes.map(p=>t('process.result',{name:p.name,pid:p.pid,result:localBuiltin(codes[p.state] || '身份核验不可用，未操作')})).join('; ')});
}
