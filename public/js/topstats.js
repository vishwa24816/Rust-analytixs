import { getJSON, statsURL } from './api.js';
import { short, pct, dur, delta } from './format.js';

const DEFS = [
  ['visitors', 'Visitors', short],
  ['visits', 'Visits', short],
  ['pageviews', 'Pageviews', short],
  ['bounce_rate', 'Bounce rate', pct],
  ['visit_duration', 'Visit duration', dur],
];

export async function render() {
  const el = document.getElementById('topstats');
  let data;
  try {
    data = await getJSON(statsURL('aggregate', { metrics: DEFS.map(d => d[0]).join(',') }));
  } catch { el.innerHTML = '<div class="stat">failed to load</div>'; return; }
  el.innerHTML = DEFS.map(([k, label, f]) => {
    const d = data.comparison ? delta(data.comparison[k]) : { t: '', c: '' };
    return `<div class="stat"><div class="v">${f(data[k])}</div><div>${label}</div><div class="d ${d.c}">${d.t}</div></div>`;
  }).join('');
}
