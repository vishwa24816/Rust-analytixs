import { getJSON, domain, statsURL } from './api.js';

// Annotation ticks under the graph for the current period.
export async function render() {
  const box = document.getElementById('annotations');
  const p = new URLSearchParams(location.search);
  const q = new URLSearchParams({ period: p.get('period') || '30d' });
  for (const k of ['date', 'from', 'to']) if (p.get(k)) q.set(k, p.get(k));
  let list;
  try {
    list = await getJSON(`/api/sites/${encodeURIComponent(domain())}/annotations?${q}`);
  } catch { return; }
  if (!list.length) { box.innerHTML = ''; return; }
  box.innerHTML = '◆ ' + list.map(a => `<span title="${escapeHtml(a.content)}">${escapeHtml(a.date)}</span>`).join(' · ');
  // + add form (admin): date + content
  box.innerHTML += ` <input id="ann-date" type="date" size="10"> <input id="ann-text" placeholder="note" size="16"> <button id="ann-add">annotate</button>`;
  document.getElementById('ann-add').addEventListener('click', async () => {
    const date = document.getElementById('ann-date').value;
    const content = document.getElementById('ann-text').value.trim();
    if (!date || !content) return;
    const fd = new FormData();
    fd.set('date', date); fd.set('content', content);
    await fetch(`/api/sites/${encodeURIComponent(domain())}/annotations`, { method: 'POST', body: new URLSearchParams(fd), credentials: 'same-origin' });
    render();
  });
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
