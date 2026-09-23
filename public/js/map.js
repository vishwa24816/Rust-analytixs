import { getJSON, statsURL } from './api.js';

// Static choropleth: pre-rendered country paths, colored by visitors.
// Needs visit:country data (requires geo lookup; empty until then).
export async function render() {
  const svg = document.getElementById('worldmap');
  let [paths, meta, data] = [{}, {}, { results: [] }];
  try {
    [paths, meta] = await Promise.all([
      getJSON('/public/world-map.json'),
      getJSON('/public/countries_meta.json'),
    ]);
    data = await getJSON(statsURL('breakdown', { property: 'visit:country', metrics: 'visitors', limit: 250 }));
  } catch { return; }
  const byA3 = {};
  for (const r of data.results || []) {
    const a2 = String(r.name || '').toUpperCase();
    const m = meta[a2];
    if (m) byA3[m[0]] = Number(r.visitors) || 0;
  }
  const max = Math.max(...Object.values(byA3), 1);
  const NS = 'http://www.w3.org/2000/svg';
  svg.innerHTML = '';
  for (const [a3, d] of Object.entries(paths)) {
    const p = document.createElementNS(NS, 'path');
    p.setAttribute('d', d);
    const v = byA3[a3] || 0;
    if (v > 0) {
      const t = v / max;
      p.style.fill = `rgb(${Math.round(79 + (239 - 79) * 0 + 0 * t)},${Math.round(70 + (180 - 70) * 0)},${Math.round(229 - (229 - 100) * t)})`;
      p.style.fill = `rgba(79,70,229,${(0.15 + 0.85 * t).toFixed(2)})`;
      p.innerHTML = `<title>${a3}: ${v}</title>`;
    }
    p.addEventListener('click', () => {
      // find alpha-2 for filter
      const a2 = Object.entries(meta).find(([, m]) => m[0] === a3)?.[0];
      if (!a2) return;
      const sp = new URLSearchParams(location.search);
      const cur = sp.get('filters');
      const f = `visit:country==${a2}`;
      sp.set('filters', cur ? cur + ';' + f : f);
      location.search = sp.toString();
    });
    svg.appendChild(p);
  }
}
