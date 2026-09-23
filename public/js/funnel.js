import { getJSON, domain } from './api.js';

export async function render() {
  const sel = document.getElementById('funnel-sel');
  const box = document.querySelector('#bd-funnel .rows');
  let list;
  try {
    list = await getJSON(`/api/sites/${encodeURIComponent(domain())}/funnels`);
  } catch { return; }
  if (!list.length) { box.innerHTML = '<div class="row">no funnels</div>'; return; }
  sel.innerHTML = list.map(f => `<option value="${f.id}">${escapeHtml(f.name)}</option>`).join('');
  const load = async () => {
    const p = new URLSearchParams(location.search);
    const q = new URLSearchParams({ period: p.get('period') || '30d' });
    const data = await getJSON(`/api/sites/${encodeURIComponent(domain())}/funnels/${sel.value}/stats?${q}`);
    const rows = data.results || [];
    const first = Math.max(...rows.map(r => r.visitors), 1);
    box.innerHTML = rows.map(r => `
      <div class="row"><span>${escapeHtml(r.step)}</span><b>${r.visitors} (${(r.dropoff_rate * 100).toFixed(0)}% drop)</b></div>
      <div class="bar" style="width:${(r.visitors / first * 100).toFixed(1)}%"></div>`).join('');
  };
  sel.addEventListener('change', load);
  await load();
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
