import { getJSON, statsURL } from './api.js';
import { short } from './format.js';

// Goal conversions: custom events with totals + conversion rate vs visitors.
export async function render() {
  const box = document.querySelector('#bd-conversions .rows');
  let agg, data;
  try {
    [agg, data] = await Promise.all([
      getJSON(statsURL('aggregate', { metrics: 'visitors' })),
      getJSON(statsURL('breakdown', { property: 'event:name', metrics: 'events,visitors', limit: 10 })),
    ]);
  } catch { box.innerHTML = '<div class="row">failed</div>'; return; }
  const visitors = Number(agg.visitors) || 0;
  const rows = (data.results || []).filter(r => r.name !== 'pageview' && r.name !== 'engagement');
  if (!rows.length) { box.innerHTML = '<div class="row">no custom events</div>'; return; }
  const max = Math.max(...rows.map(r => Number(r.events) || 0), 1);
  box.innerHTML = rows.map(r => {
    const cr = visitors ? (Number(r.events) / visitors * 100).toFixed(1) + '%' : '–';
    return `<div class="row"><span>${escapeHtml(String(r.name))}</span><b>${short(r.events)} · ${cr}</b></div>
    <div class="bar" style="width:${(Number(r.events) / max * 100).toFixed(1)}%"></div>`;
  }).join('');
}

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
