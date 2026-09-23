import { getJSON, domain } from './api.js';

// Saved segments → one click applies their stored filters.
export async function render() {
  const box = document.getElementById('segments');
  let list;
  try {
    list = await getJSON(`/api/sites/${encodeURIComponent(domain())}/segments`);
  } catch { return; }
  if (!list.length) { box.innerHTML = ''; return; }
  box.innerHTML = 'Segments: ' + list.map(s =>
    `<button class="seg" data-f="${escapeAttr(s.filters)}">${escapeHtml(s.name)}</button>`).join(' ');
  box.querySelectorAll('.seg').forEach(b => b.addEventListener('click', () => {
    const p = new URLSearchParams(location.search);
    if (b.dataset.f) p.set('filters', b.dataset.f); else p.delete('filters');
    location.search = p.toString();
  }));
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
function escapeAttr(s) { return escapeHtml(s); }
