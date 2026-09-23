export function short(n) {
  if (n == null) return '–';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'k';
  if (Number.isInteger(n)) return String(n);
  return Number(n).toFixed(2);
}
export function pct(x) { return x == null ? '–' : (Number(x) * 100).toFixed(1) + '%'; }
export function dur(s) {
  if (s == null) return '–';
  s = Math.round(Number(s));
  if (s < 60) return s + 's';
  const m = Math.floor(s / 60);
  return m + 'm ' + (s % 60) + 's';
}
export function delta(x) {
  if (x == null) return { t: '–', c: '' };
  const r = Math.round(Number(x));
  return { t: (r >= 0 ? '+' : '') + r + '%', c: r >= 0 ? 'up' : 'down' };
}
