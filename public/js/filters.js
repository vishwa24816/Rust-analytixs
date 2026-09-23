import { getJSON, domain } from './api.js';

const DIMS = ['visit:source', 'visit:browser', 'visit:device', 'visit:country', 'visit:os', 'visit:utm_source', 'visit:utm_medium', 'visit:utm_campaign', 'event:page', 'event:name'];

export function render() {
  const box = document.getElementById('filters');
  const p = new URLSearchParams(location.search);
  const cur = (p.get('filters') || '').split(';').filter(Boolean);
  box.innerHTML =
    cur.map(f => `<span class="fpill" data-f="${escapeAttr(f)}">${escapeHtml(f)} ✕</span>`).join('') +
    `<select id="fdim">${DIMS.map(d => `<option>${d}</option>`).join('')}</select>
     <input id="fval" placeholder="value" size="12" list="fsuggest">
     <datalist id="fsuggest"></datalist>
     <button id="fadd">add filter</button>`;
  box.querySelectorAll('.fpill').forEach(el => el.addEventListener('click', () => {
    const rest = cur.filter(f => f !== el.dataset.f);
    rest.length ? p.set('filters', rest.join(';')) : p.delete('filters');
    location.search = p.toString();
  }));
  const dimSel = document.getElementById('fdim');
  const valInput = document.getElementById('fval');
  const dl = document.getElementById('fsuggest');
  let t;
  async function suggest() {
    const dim = dimSel.value;
    const q = valInput.value.trim();
    clearTimeout(t);
    t = setTimeout(async () => {
      try {
        const vals = await getJSON(`/api/sites/${encodeURIComponent(domain())}/suggestions?dimension=${encodeURIComponent(dim)}&q=${encodeURIComponent(q)}`);
        dl.innerHTML = vals.map(v => `<option value="${escapeAttr(String(v))}">`).join('');
      } catch { /* ignore */ }
    }, 200);
  }
  dimSel.addEventListener('change', suggest);
  valInput.addEventListener('input', suggest);
  suggest();
  document.getElementById('fadd').addEventListener('click', () => {
    const v = valInput.value.trim();
    if (!v) return;
    const f = `${dimSel.value}==${v}`;
    p.set('filters', cur.concat(f).join(';'));
    location.search = p.toString();
  });
}

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
function escapeAttr(s) { return escapeHtml(s); }
