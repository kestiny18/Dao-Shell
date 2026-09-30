let homeSnapshot = null;
let processSort = 'cpu';
function resourcePercent(value) { return value == null || !Number.isFinite(value) ? '未知' : value > 0 && value < 0.1 ? '<0.1%' : `${value.toFixed(1)}%`; }
function renderProcesses() {
  const snapshot = homeSnapshot;
  const processes = snapshot?.process_rankings?.[processSort] || [];
  $('resource-processes').replaceChildren();
  document.querySelectorAll('[data-process-sort]').forEach(button => button.setAttribute('aria-pressed', String(button.dataset.processSort === processSort)));
  for (const process of processes) {
    const row = textNode('tr', '');
    row.append(textNode('td', `${process.name || '未知名称'} / ${process.pid}`), textNode('td', resourcePercent(process.cpu_percent_total)), textNode('td', process.memory_bytes == null ? '未知' : humanBytes(process.memory_bytes)));
    $('resource-processes').append(row);
  }
  $('resource-empty').hidden = processes.length > 0;
  $('resource-sample').textContent = snapshot ? `采样时间：${new Date(snapshot.observed_at).toLocaleString()} · CPU 计数窗口：${Number.isFinite(snapshot.cpu_interval_ms) ? (snapshot.cpu_interval_ms / 1000).toFixed(2) + ' 秒' : '未知'} · 仅手动刷新，不记录趋势。` : '等待采样';
  $('resource-coverage').textContent = `本次枚举 ${snapshot?.matched_processes ?? '未知数量'} 个进程，显示${processSort === 'cpu' ? ' CPU' : '内存'}占用前 ${processes.length} 项。${snapshot?.disappeared_processes ?? '未知数量'} 个进程在两次观测间消失或身份变化，未列入；新出现或无法读取的进程可能缺少 CPU 数据。排序仅比较本次可读值，未知排在末尾。`;
}
let appMenu = null;
function closeAppMenu() { appMenu?.remove(); appMenu = null; }
async function applicationAction(app, action) {
  closeAppMenu();
  if (!api || busy || !app.action_id) return;
  if (action !== 'uninstall' && app.action_reason) { $('activity').textContent = app.action_reason; return; }
  applicationBusy = true; setBusy(true); $('activity').textContent = action === 'admin' ? '等待 Windows 管理员确认…' : '正在请求 Windows…';
  try { const reply = await api.invoke('application_action', {appId:app.action_id, action}); $('activity').textContent = reply.message; }
  catch (error) { $('activity').textContent = String(error); }
  finally { applicationBusy = false; setBusy(false); }
}
function showAppMenu(app, row, event) {
  event.preventDefault(); closeAppMenu();
  const menu = textNode('div', '', 'app-context-menu'); appMenu = menu;
  menu.setAttribute('role', 'menu'); menu.setAttribute('aria-label', `${app.name}的操作`);
  for (const [action, label] of [['run','运行'],['admin','以管理员身份运行'],['location','打开文件位置'],['uninstall','卸载（打开系统设置）']]) {
    const button = textNode('button', label); button.type = 'button'; button.setAttribute('role','menuitem');
    const reason = busy ? '请等待当前操作完成' : !app.action_id ? '请刷新概览以获取操作入口' : action !== 'uninstall' ? app.action_reason : '';
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
  if (value == null || !Number.isFinite(value)) return '暂不可用';
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
    row.tabIndex = 0; row.setAttribute('aria-label', `${app.name}，右键或 Shift+F10 查看操作`);
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
    info.append(textNode('strong', app.name), textNode('small', [app.version, app.publisher].filter(Boolean).join(' · ') || '版本信息未登记'));
    info.append(textNode('small', app.running === true ? '运行中' : '运行状态未知', app.running === true ? 'running-badge' : ''));
    info.append(textNode('small', `估算安装占用：${app.estimated_size_bytes == null ? '未知' : humanBytes(app.estimated_size_bytes)}`));
    row.append(icon, info);
    $('applications').append(row);
  }
  if (!matches.length) $('applications').append(textNode('p', '当前范围内没有匹配的应用记录。'));
}
function renderOverview(snapshot) {
  homeSnapshot = snapshot; const system = snapshot.system;
  $('device-summary').textContent = [snapshot.device.host, snapshot.device.os].filter(Boolean).join(' · ');
  $('cpu-value').textContent = system.cpu_percent == null ? '暂不可用' : `${system.cpu_percent.toFixed(1)}%`;
  $('memory-value').textContent = humanBytes(system.memory_used_bytes);
  $('memory-detail').textContent = `总计 ${humanBytes(system.memory_total_bytes)} · 可用 ${humanBytes(system.memory_available_bytes)}`;
  $('uptime-value').textContent = `${Math.floor(snapshot.device.uptime_seconds / 3600)} 小时`;
  $('architecture').textContent = `${snapshot.device.architecture || '未知架构'} · ${snapshot.logical_cpus} 个逻辑处理器`;
  $('observed-at').textContent = `采样于 ${new Date(snapshot.observed_at).toLocaleTimeString()}`;
  $('disks').replaceChildren();
  for (const disk of system.disks || []) {
    const card = textNode('article', '', 'card disk');
    const used = disk.total_bytes ? Math.max(0, Math.min(100, 100 * (1 - disk.available_bytes / disk.total_bytes))) : null;
    const meter = document.createElement('progress'); meter.max = 100; if (used != null) meter.value = used;
    card.append(textNode('strong', pathText(disk.mount)), textNode('p', `可用 ${humanBytes(disk.available_bytes)} / ${humanBytes(disk.total_bytes)}`), meter);
    $('disks').append(card);
  }
  if (!system.disks?.length) $('disks').append(textNode('p', '本次没有可用的磁盘容量数据。'));
  $('networks').replaceChildren(); $('network-count').textContent = `· ${snapshot.network.adapters.length} 个`;
  $('network-note').textContent = snapshot.network.limits;
  for (const adapter of snapshot.network.adapters) {
    const row = textNode('article', '', 'network-card');
    row.append(textNode('strong', adapter.name), textNode('small', `${adapter.has_ip ? '已配置 IP' : '未观测到 IP'} · ↓ ${humanBytes(adapter.received_bytes_per_second)}/s · ↑ ${humanBytes(adapter.transmitted_bytes_per_second)}/s`));
    $('networks').append(row);
  }
  $('app-count').textContent = snapshot.applications.available ? `· ${snapshot.applications.items.length} 项记录` : '· 暂不可用';
  $('app-note').textContent = snapshot.applications.limits + (snapshot.applications.observed_at ? ` 应用采样于 ${new Date(snapshot.applications.observed_at).toLocaleTimeString()}。` : '') + (snapshot.applications.unreadable_process_paths ? ` ${snapshot.applications.unreadable_process_paths} 个进程路径不可读，可能受系统权限限制。` : '') + (snapshot.applications.partial ? ' 部分注册表位置读取失败或达到枚举上限。' : '');
  renderProcesses(); renderApps(); $('explain-home').disabled = busy;
}
async function refreshOverview() {
  if (!api || busy) return;
  closeAppMenu();
  setBusy(true); $('activity').textContent = '正在读取本机概览…';
  try { renderOverview(await api.invoke('computer_overview')); $('activity').textContent = '本机概览已更新'; }
  catch (error) { $('activity').textContent = String(error); $('device-summary').textContent = homeSnapshot ? '刷新失败，下面保留上一次采样。' : '本次概览暂不可用，请稍后刷新。'; }
  finally { setBusy(false); $('explain-home').disabled = !homeSnapshot; }
}
$('refresh-home').addEventListener('click', refreshOverview);
document.querySelectorAll('[data-process-sort]').forEach(button => button.addEventListener('click', () => { processSort = button.dataset.processSort; renderProcesses(); }));
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
  const label = value => typeof value === 'string' && value.trim() ? value : '未知';
  const names = values => Array.isArray(values) && values.length ? values.map(label).join('；') : '未知';
  const size = value => Number.isFinite(value) && value > 0 ? humanBytes(value) : '未知';
  const volumes = Array.isArray(profile.volumes) && profile.volumes.length ? profile.volumes.map((bytes, i) => `卷 ${i + 1}：${size(bytes)}`).join('；') : '未知';
  return [`品牌：${label(profile.brand)}`, `型号：${label(profile.model)}`, `操作系统：${label(profile.os)}`, `CPU：${names(profile.cpus)}`, `总内存：${size(profile.memory_bytes)}`, `显示适配器：${names(profile.graphics)}`, `卷容量（不是物理硬盘，不累加）：${volumes}`, `采集时间：${label(profile.observed_at)}`].join('\n');
}
async function refreshProfile() {
  if (!api || busy) { $('profile-status').textContent = '请等待当前操作完成后读取档案。'; return; }
  setBusy(true); $('profile-status').textContent = '正在读取本机电脑档案…';
  try {
    const profile = await api.invoke('computer_profile');
    profileText = profileSummary(profile); profileSnapshot = profile;
    $('profile-summary').textContent = profileText;
    $('profile-status').textContent = '已读取；复制仅包含下方摘要，不含主机名、用户名、序列号、地址或私人路径。';
  } catch (error) { $('profile-status').textContent = `读取失败：${error}${profileSnapshot ? '；保留上次摘要和采集时间。' : ''}`; }
  finally { setBusy(false); }
}
$('computer-profile').addEventListener('toggle', () => { if ($('computer-profile').open && !profileSnapshot) refreshProfile(); });
$('refresh-profile').addEventListener('click', refreshProfile);
$('copy-profile').addEventListener('click', async () => {
  if (!profileText) { $('profile-status').textContent = '请先读取电脑档案。'; return; }
  try {
    if (!navigator.clipboard?.writeText) throw Error('当前环境不支持剪贴板，请选择下方摘要手动复制');
    await navigator.clipboard.writeText(profileText);
    $('profile-status').textContent = '摘要已复制。';
  } catch (error) { $('profile-status').textContent = `复制失败：${error}`; }
});
