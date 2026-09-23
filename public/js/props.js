import { getJSON, statsURL } from './api.js';
import { short } from './format.js';

// Custom property explorer: enter a prop key, see value breakdown.
export function render() {
  document.getElementById('props-go').addEventListener('click', load);
}

async function load() {
  const box = document.querySelector('#bd-props .rows');
  const key = document.getElementById('props-key').value.trim();
  if (!key) { box.innerHTML = '<div class="row">enter a property key</div>'; return; }
  const property = `event:props:${key}`;
  let data;
  try {
    data = await getJSON(statsURL('breakdown', { property, metrics: 'events,visitors', limit: 10 }));
  } catch { box.innerHTML = '<div class="row">failed</div>'; return; }
  const rows = data.results || [];
  if (!rows.length) { box.innerHTML = '<div class="row">no data</div>'; return; }
  const max = Math.max(...rows.map(r => Number(r.events) || 0), 1);
  box.innerHTML = rows.map(r => `
    <div class="row"><span>${escapeHtml(String(r.name))}</span><b>${short(r.events)} events</b></div>
    <div class="bar" style="width:${(Number(r.events) / max * 100).toFixed(1)}%"></div>`).join('');
}

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
