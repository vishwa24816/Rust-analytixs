import { getJSON, domain } from './api.js';

export function start() {
  const el = document.getElementById('realtime');
  const tick = async () => {
    try {
      const n = await getJSON(`/api/v1/stats/realtime/visitors?site_id=${encodeURIComponent(domain())}`);
      el.textContent = `● ${n} online`;
      document.title = `(${n}) ${domain()}`;
    } catch { /* keep last value */ }
  };
  tick();
  setInterval(tick, 5000);
}
