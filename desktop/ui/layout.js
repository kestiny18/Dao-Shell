/* Window-local presentation preferences never change access or model settings. */
function readPreference(key, fallback) {
  try { return localStorage.getItem(key) ?? fallback; } catch { return fallback; }
}
function savePreference(key, value) {
  try { localStorage.setItem(key, value); } catch { /* A read-only profile still works. */ }
}
const divider = $('sidebar-resize');
let sidebarWidth = Number(readPreference('dao.sidebarWidth', 205));
function resizeSidebar(width, persist = false) {
  const maximum = Math.max(170, Math.min(360, window.innerWidth - 460));
  sidebarWidth = Math.round(Math.max(170, Math.min(maximum, Number.isFinite(width) ? width : 205)));
  document.querySelector('.shell').style.setProperty('--sidebar-width', `${sidebarWidth}px`);
  divider.setAttribute('aria-valuemax', String(maximum));
  divider.setAttribute('aria-valuenow', String(sidebarWidth));
  if (persist) savePreference('dao.sidebarWidth', sidebarWidth);
}
let dragPointer = null;
divider.addEventListener('pointerdown', (event) => {
  if (event.button !== 0) return;
  event.preventDefault(); dragPointer = event.pointerId;
  divider.setPointerCapture(event.pointerId); document.body.classList.add('resizing-sidebar');
});
divider.addEventListener('pointermove', (event) => {
  if (event.pointerId === dragPointer) resizeSidebar(event.clientX - document.querySelector('.shell').getBoundingClientRect().left);
});
function stopResize() {
  if (dragPointer === null) return;
  dragPointer = null; document.body.classList.remove('resizing-sidebar'); savePreference('dao.sidebarWidth', sidebarWidth);
}
divider.addEventListener('pointerup', stopResize);
divider.addEventListener('pointercancel', stopResize);
divider.addEventListener('lostpointercapture', stopResize);
divider.addEventListener('dblclick', () => resizeSidebar(205, true));
divider.addEventListener('keydown', (event) => {
  const width = { ArrowLeft:sidebarWidth - 10, ArrowRight:sidebarWidth + 10, Home:170, End:360 }[event.key];
  if (width !== undefined) { event.preventDefault(); resizeSidebar(width, true); }
});
window.addEventListener('resize', () => resizeSidebar(sidebarWidth));
resizeSidebar(sidebarWidth);
