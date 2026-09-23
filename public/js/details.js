import { getJSON, statsURL } from './api.js';
import { short } from './format.js';
import { open } from './modal.js';

// Drilldown modal: searchable, paginated breakdown with CSV export.
export async function details(title, property, metric = 'visitors') {
  const wrap = document.createElement('div');
  wrap.innerHTML = `
    <input class="dsearch" placeholder="search..." size="20">
    <a class="csv" href="#">export csv</a>
    <div class="rows"></div>
    <div class="pager"><button class="prev">← prev</button><span class="pg"></span><button class="next">next →</button></div>`;
  open(title, wrap);
  const box = wrap.querySelector('.rows');
  const search = wrap.querySelector('.dsearch');
  const pg = wrap.querySelector('.pg');
  let page = 1;
  const limit = 25;

  async function load() {
    const extra = { property, metrics: metric, limit, page };
    const q = search.value.trim();
    if (q) extra.search = q;
    let data;
    try {
      data = await getJSON(statsURL('breakdown', extra));
    } catch { box.innerHTML = '<div class="row">failed</div>'; return; }
    const rows = data.results || [];
    const max = Math.max(...rows.map(r => Number(r[metric]) || 0), 1);
    box.innerHTML = rows.map(r => `
      <div class="row" data-dim="${property}" data-val="${escapeHtml(String(r.name))}">
        <span>${escapeHtml(String(r.name))}</span><b>${short(r[metric])}</b>
      </div><div class="bar" style="width:${(Number(r[metric]) / max * 100).toFixed(1)}%"></div>`).join('')
      || '<div class="row">no data</div>';
    pg.textContent = 'page ' + page;
    box.querySelectorAll('.row[data-val]').forEach(el => el.addEventListener('click', () => {
      const f = `${el.dataset.dim}==${el.dataset.val}`;
      const p = new URLSearchParams(location.search);
      const cur = p.get('filters');
      p.set('filters', cur ? cur + ';' + f : f);
      location.search = p.toString();
    }));
    wrap.querySelector('.csv').href = statsURL('csv', { property, metrics: metric });
  }

  let t;
  search.addEventListener('input', () => { clearTimeout(t); t = setTimeout(() => { page = 1; load(); }, 250); });
  wrap.querySelector('.prev').addEventListener('click', () => { if (page > 1) { page--; load(); } });
  wrap.querySelector('.next').addEventListener('click', () => { page++; load(); });
  await load();
}

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
