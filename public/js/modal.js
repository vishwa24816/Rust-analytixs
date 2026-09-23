// Fullscreen details modal shell.
export function open(title, bodyEl) {
  const root = document.getElementById('modal-root');
  root.innerHTML = `<div class="modal-bg"><div class="modal">
    <div class="modal-head"><h3></h3><button class="modal-x">✕</button></div>
    <div class="modal-body"></div></div></div>`;
  root.querySelector('h3').textContent = title;
  root.querySelector('.modal-body').appendChild(bodyEl);
  const close = () => { root.innerHTML = ''; };
  root.querySelector('.modal-x').addEventListener('click', close);
  root.querySelector('.modal-bg').addEventListener('click', e => {
    if (e.target.classList.contains('modal-bg')) close();
  });
  document.addEventListener('keydown', function esc(e) {
    if (e.key === 'Escape') { close(); document.removeEventListener('keydown', esc); }
  });
  return close;
}
