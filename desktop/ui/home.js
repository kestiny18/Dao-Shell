let homeSnapshot = null;
let appView = readPreference('dao.appView', 'cards') === 'list' ? 'list' : 'cards';
function humanBytes(value) {
  if (value == null || !Number.isFinite(value)) return '暂不可用';
  const units = ['B','KiB','MiB','GiB','TiB']; let index = 0;
  while (value >= 1024 && index < units.length - 1) { value /= 1024; index++; }
  return `${value.toFixed(index ? 1 : 0)} ${units[index]}`;
}
function textNode(tag, text, className) { const node = document.createElement(tag); node.textContent = text; if (className) node.className = className; return node; }
function renderApps() {
  const query = $('app-filter').value.trim().toLowerCase();
  const matches = (homeSnapshot?.applications.items || []).filter((item) => item.name.toLowerCase().includes(query));
  $('applications').replaceChildren();
  $('applications').dataset.view = appView;
  document.querySelectorAll('[data-app-view]').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.appView === appView)));
  for (const app of matches.slice(0, 100)) {
    const row = textNode('article', '', 'app-item');
    const initials = Array.from(app.name.trim()).slice(0, 2).join('').toUpperCase();
    const icon = textNode('div', initials, 'app-icon'); icon.setAttribute('aria-hidden', 'true');
    if (typeof app.icon === 'string' && app.icon.length < 24000 && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(app.icon)) {
      const image = document.createElement('img'); image.alt = ''; image.src = app.icon;
      image.addEventListener('error', () => icon.replaceChildren(document.createTextNode(initials)), { once:true });
      icon.replaceChildren(image);
    }
    const info = textNode('div', '', 'app-info');
    info.append(textNode('strong', app.name), textNode('small', [app.version, app.publisher].filter(Boolean).join(' · ') || '版本信息未登记'));
    row.append(icon, info);
    $('applications').append(row);
  }
  if (matches.length > 100) $('applications').append(textNode('p', `显示前 100 项，共匹配 ${matches.length} 项；请缩小筛选。`));
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
    const row = textNode('div', '', 'observation-row');
    row.append(textNode('strong', adapter.name), textNode('small', `${adapter.has_ip ? '已配置 IP' : '未观测到 IP'} · ↓ ${humanBytes(adapter.received_bytes_per_second)}/s · ↑ ${humanBytes(adapter.transmitted_bytes_per_second)}/s`));
    $('networks').append(row);
  }
  $('app-count').textContent = snapshot.applications.available ? `· ${snapshot.applications.items.length} 项记录` : '· 暂不可用';
  $('app-note').textContent = snapshot.applications.limits + (snapshot.applications.partial ? ' 部分注册表位置读取失败或达到枚举上限。' : '');
  renderApps(); $('explain-home').disabled = busy;
}
async function refreshOverview() {
  if (!api || busy) return;
  setBusy(true); $('activity').textContent = '正在读取本机概览…';
  try { renderOverview(await api.invoke('computer_overview')); $('activity').textContent = '本机概览已更新'; }
  catch (error) { $('activity').textContent = String(error); $('device-summary').textContent = homeSnapshot ? '刷新失败，下面保留上一次采样。' : '本次概览暂不可用，请稍后刷新。'; }
  finally { setBusy(false); $('explain-home').disabled = !homeSnapshot; }
}
$('refresh-home').addEventListener('click', refreshOverview);
$('app-filter').addEventListener('input', renderApps);
document.querySelectorAll('[data-app-view]').forEach((button) => button.addEventListener('click', () => {
  appView = button.dataset.appView; savePreference('dao.appView', appView); renderApps();
}));
$('explain-home').addEventListener('click', () => { if (homeSnapshot) run('explain'); });
refreshOverview();
