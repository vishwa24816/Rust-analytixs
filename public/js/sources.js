import { table } from './breakdown.js';

// Sources card tabs: source / referrer / UTM dimensions.
const TABS = [
  ['visit:source', 'sources'],
  ['visit:referrer', 'referrers'],
  ['visit:utm_medium', 'medium'],
  ['visit:utm_source', 'utm source'],
  ['visit:utm_campaign', 'campaign'],
];

export async function render() {
  const bar = document.getElementById('sources-tabs');
  const saved = sessionStorage.getItem('sources-tab') || TABS[0][0];
  bar.innerHTML = TABS.map(([dim, label]) =>
    `<button data-dim="${dim}" class="${dim === saved ? 'active' : ''}">${label}</button>`).join(' ');
  async function load(dim) {
    sessionStorage.setItem('sources-tab', dim);
    bar.querySelectorAll('button').forEach(b => b.classList.toggle('active', b.dataset.dim === dim));
    await table('bd-sources', dim, 'visitors', 10, 'Sources');
  }
  bar.querySelectorAll('button').forEach(b => b.addEventListener('click', () => load(b.dataset.dim)));
  await load(saved);
}
