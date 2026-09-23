import { getJSON, domain } from './api.js';

export function render() {
  const box = document.querySelector('#bd-keywords .rows');
  box.innerHTML = '<div class="row"><button id="kw-load">load keywords</button></div>';
  document.getElementById('kw-load').addEventListener('click', load);
}

async function load() {
  const box = document.querySelector('#bd-keywords .rows');
  const p = new URLSearchParams(location.search);
  const q = new URLSearchParams({ period: p.get('period') || '30d' });
  let data;
  try {
    const r = await fetch(`/api/sites/${encodeURIComponent(domain())}/search-console/keywords?${q}`, { credentials: 'same-origin' });
    if (!r.ok) {
      const t = await r.text();
      box.innerHTML = `<div class="row">${escapeHtml(shortErr(t))}</div>`;
      return;
    }
    data = await r.json();
  } catch { box.innerHTML = '<div class="row">failed</div>'; return; }
  const rows = data.rows || [];
  if (!rows.length) { box.innerHTML = '<div class="row">no data</div>'; return; }
  box.innerHTML = rows.slice(0, 10).map(r =>
    `<div class="row"><span>${escapeHtml(r.keys?.[0] || '?')}</span><b>${r.clicks || 0} clicks</b></div>`).join('');
}

function shortErr(t) {
  try { return JSON.parse(t).error || t; } catch { return t; }
}
function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
