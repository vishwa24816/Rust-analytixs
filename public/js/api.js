export async function getJSON(path) {
  const r = await fetch(path, { credentials: 'same-origin' });
  if (r.status === 401) { location.href = '/login'; throw new Error('login'); }
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}
export const domain = () => document.body.dataset.domain;
export function statsURL(endpoint, extra = {}) {
  const p = new URLSearchParams(location.search);
  const q = new URLSearchParams({ site_id: domain(), period: p.get('period') || '30d' });
  for (const k of ['date', 'from', 'to', 'filters']) if (p.get(k)) q.set(k, p.get(k));
  if (p.get('compare') === 'previous_period') q.set('compare', 'previous_period');
  for (const [k, v] of Object.entries(extra)) q.set(k, v);
  return `/api/v1/stats/${endpoint}?${q}`;
}
