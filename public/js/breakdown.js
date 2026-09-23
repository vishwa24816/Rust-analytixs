import { getJSON, statsURL } from './api.js';
import { short } from './format.js';
import { details } from './details.js';

// Renders a breakdown table into cardEl .rows; click a row to filter by it.
// Appends a "more →" link opening the full drilldown modal.
export async function table(cardId, property, metric = 'visitors', limit = 10, title, skip = []) {
  const card = document.getElementById(cardId);
  const box = card.querySelector('.rows');
  let data;
  try {
    data = await getJSON(statsURL('breakdown', { property, metrics: `${metric},visitors`, limit }));
  } catch { box.innerHTML = '<div class="row">failed</div>'; return; }
  const rows = (data.results || []).filter(r => !skip.includes(r.name));
  if (!rows.length) { box.innerHTML = '<div class="row">no data</div>'; return; }
  const max = Math.max(...rows.map(r => Number(r[metric]) || 0), 1);
  box.innerHTML = rows.slice(0, limit).map(r => `
    <div class="row" data-dim="${property}" data-val="${escapeHtml(String(r.name))}">
      <span>${escapeHtml(String(r.name))}</span><b>${short(r[metric])}</b>
    </div><div class="bar" style="width:${(Number(r[metric]) / max * 100).toFixed(1)}%"></div>`).join('')
    + `<div class="row more"><span>more →</span><span></span></div>`;
  box.querySelectorAll('.row[data-val]').forEach(el => el.addEventListener('click', () => {
    const f = `${el.dataset.dim}==${el.dataset.val}`;
    const p = new URLSearchParams(location.search);
    const cur = p.get('filters');
    p.set('filters', cur ? cur + ';' + f : f);
    location.search = p.toString();
  }));
  box.querySelector('.more').addEventListener('click', () => details(title || property, property, metric));
}

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
