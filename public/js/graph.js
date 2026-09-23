import { getJSON, statsURL } from './api.js';

export function metric() {
  return document.getElementById('graph-metric').value || 'visitors';
}

export async function draw() {
  const cv = document.getElementById('graph');
  const ctx = cv.getContext('2d');
  ctx.clearRect(0, 0, cv.width, cv.height);
  const m = metric();
  const iv = document.getElementById('graph-interval').value;
  const extra = { metrics: m };
  if (iv) extra.interval = iv;
  let rows;
  try {
    ({ results: rows } = await getJSON(statsURL('timeseries', extra)));
  } catch { return; }
  if (!rows.length) return;
  const vals = rows.map(r => Number(r[m]) || 0);
  const max = Math.max(...vals, 1);
  const W = cv.width, H = cv.height, P = 28;
  ctx.strokeStyle = '#e5e5e5'; ctx.beginPath();
  ctx.moveTo(P, 8); ctx.lineTo(P, H - P); ctx.lineTo(W - 8, H - P); ctx.stroke();
  ctx.fillStyle = '#666'; ctx.font = '11px system-ui';
  ctx.fillText(fmtTick(Math.max(...vals)), 2, 16);
  ctx.fillText(String(rows[0].date), P, H - 8);
  const last = String(rows[rows.length - 1].date);
  ctx.fillText(last, W - 8 - ctx.measureText(last).width, H - 8);
  const X = i => P + (i / Math.max(rows.length - 1, 1)) * (W - P - 8);
  const Y = v => (H - P) - (v / max) * (H - P - 16);
  ctx.beginPath();
  ctx.moveTo(X(0), Y(vals[0]));
  vals.forEach((v, i) => ctx.lineTo(X(i), Y(v)));
  ctx.strokeStyle = '#4f46e5'; ctx.lineWidth = 2; ctx.stroke();
  ctx.lineTo(X(vals.length - 1), H - P); ctx.lineTo(X(0), H - P); ctx.closePath();
  ctx.fillStyle = 'rgba(79,70,229,.12)'; ctx.fill();
  // hover readout
  cv.onmousemove = e => {
    const r = cv.getBoundingClientRect();
    const i = Math.round((e.clientX - r.left) / r.width * W - P < 0 ? 0 : ((e.clientX - r.left) / r.width * (W - P - 8)) / ((W - P - 8) / Math.max(rows.length - 1, 1)));
    const j = Math.max(0, Math.min(rows.length - 1, i));
    cv.title = `${rows[j].date}: ${fmtTick(vals[j])} ${m}`;
  };
}

function fmtTick(v) {
  if (v >= 1000) return (v / 1000).toFixed(1) + 'k';
  if (!Number.isInteger(v)) return v.toFixed(2);
  return String(v);
}
