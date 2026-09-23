import { draw } from './graph.js';

export function render(onChange) {
  const p = new URLSearchParams(location.search);
  const cur = p.get('period') || '30d';
  document.querySelectorAll('#periods button[data-period]').forEach(b => {
    b.classList.toggle('active', b.dataset.period === cur);
    b.addEventListener('click', () => {
      p.set('period', b.dataset.period);
      p.delete('date'); p.delete('from'); p.delete('to');
      history.replaceState(null, '', location.pathname + '?' + p.toString());
      syncCustom();
      onChange();
    });
  });
  const cmp = document.getElementById('compare');
  cmp.checked = p.get('compare') === 'previous_period';
  cmp.addEventListener('change', () => {
    cmp.checked ? p.set('compare', 'previous_period') : p.delete('compare');
    history.replaceState(null, '', location.pathname + '?' + p.toString());
    onChange();
  });
  syncCustom();
  document.getElementById('custom-apply').addEventListener('click', () => {
    const from = document.getElementById('from').value;
    const to = document.getElementById('to').value;
    if (!from || !to) return;
    p.set('period', 'custom');
    p.set('from', from); p.set('to', to);
    p.delete('date');
    history.replaceState(null, '', location.pathname + '?' + p.toString());
    onChange();
  });
  document.getElementById('graph-metric').addEventListener('change', () => draw());
  document.getElementById('graph-interval').addEventListener('change', () => draw());
}

function syncCustom() {
  const p = new URLSearchParams(location.search);
  if (p.get('from')) document.getElementById('from').value = p.get('from');
  if (p.get('to')) document.getElementById('to').value = p.get('to');
}
