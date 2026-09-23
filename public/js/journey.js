import { getJSON, domain } from './api.js';

export function render() {
  document.getElementById('journey-go').addEventListener('click', load);
}

async function load() {
  const box = document.querySelector('#bd-journey .rows');
  const entry = document.getElementById('journey-entry').value || '/';
  const p = new URLSearchParams(location.search);
  const q = new URLSearchParams({ period: p.get('period') || '30d', entry });
  let data;
  try {
    data = await getJSON(`/api/sites/${encodeURIComponent(domain())}/journey?${q}`);
  } catch { box.innerHTML = '<div class="row">failed</div>'; return; }
  const rows = data.results || [];
  if (!rows.length) { box.innerHTML = '<div class="row">no journeys</div>'; return; }
  const max = Math.max(...rows.map(r => r.visitors), 1);
  box.innerHTML = rows.map(r => `
    <div class="row"><span>→ ${escapeHtml(r.path)}</span><b>${r.visitors}</b></div>
    <div class="bar" style="width:${(r.visitors / max * 100).toFixed(1)}%"></div>`).join('');
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
