/*
 * The desk in the hero: pick a side and the monitor moves there, the edge it
 * shares with the laptop lights up, and a pointer crosses it. The same chain
 * maths as the app, for one monitor, centred.
 */

const VIEW = { width: 760, height: 420 };
const LAPTOP = { width: 210, height: 135, base: 17 };
const MONITOR = { width: 300, height: 180, stand: 36 };
const EDGES = {
  left: ['left', 'right'],
  right: ['right', 'left'],
  above: ['top', 'bottom'],
  below: ['bottom', 'top'],
};

const svg = document.getElementById('desk');
const monitor = document.getElementById('monitor');
const laptop = document.getElementById('laptop');
const edge = document.getElementById('edge');
const cursor = document.getElementById('cursor');
const caption = document.getElementById('demo-caption');
const buttons = document.querySelectorAll('.demo [data-side]');
const reduced = window.matchMedia('(prefers-reduced-motion: reduce)');

function place(side) {
  const m =
    side === 'left' ? { x: -MONITOR.width, y: (LAPTOP.height - MONITOR.height) / 2 }
    : side === 'right' ? { x: LAPTOP.width, y: (LAPTOP.height - MONITOR.height) / 2 }
    : side === 'above' ? { x: (LAPTOP.width - MONITOR.width) / 2, y: -MONITOR.height }
    : { x: (LAPTOP.width - MONITOR.width) / 2, y: LAPTOP.height };
  const standBelowMonitor = side !== 'above';
  const baseBelowLaptop = side !== 'below';

  // Centre everything in the view, stands included.
  const minX = Math.min(0, m.x);
  const minY = Math.min(0, m.y);
  const maxX = Math.max(LAPTOP.width, m.x + MONITOR.width);
  const maxY = Math.max(
    LAPTOP.height + (baseBelowLaptop ? LAPTOP.base : 0),
    m.y + MONITOR.height + (standBelowMonitor ? MONITOR.stand : 0),
  );
  const dx = (VIEW.width - (maxX - minX)) / 2 - minX;
  const dy = (VIEW.height - (maxY - minY)) / 2 - minY;

  laptop.style.transform = `translate(${dx}px, ${dy}px)`;
  monitor.style.transform = `translate(${m.x + dx}px, ${m.y + dy}px)`;
  monitor.querySelector('.stand').classList.toggle('hidden', !standBelowMonitor);
  laptop.querySelector('.stand').classList.toggle('hidden', !baseBelowLaptop);

  // The shared edge, in laptop coordinates.
  const horizontal = side === 'left' || side === 'right';
  const x = side === 'left' ? 0 : side === 'right' ? LAPTOP.width : null;
  const y = side === 'above' ? 0 : side === 'below' ? LAPTOP.height : null;
  const inset = 6;
  const line = horizontal
    ? { x1: x, y1: inset, x2: x, y2: LAPTOP.height - inset }
    : { x1: inset, y1: y, x2: LAPTOP.width - inset, y2: y };
  for (const l of edge.querySelectorAll('line')) {
    for (const [k, v] of Object.entries(line)) l.setAttribute(k, v);
  }
  edge.style.transform = `translate(${dx}px, ${dy}px)`;

  // The pointer: from the middle of the laptop, across the edge, into the monitor.
  const mid = { x: (line.x1 + line.x2) / 2 + dx, y: (line.y1 + line.y2) / 2 + dy };
  const from = { x: dx + LAPTOP.width / 2, y: dy + LAPTOP.height / 2 };
  const to = { x: m.x + dx + MONITOR.width / 2, y: m.y + dy + MONITOR.height / 2 };
  cursor.getAnimations().forEach((a) => a.cancel());
  if (reduced.matches) {
    cursor.style.transform = `translate(${mid.x}px, ${mid.y}px)`;
  } else {
    const at = (p) => ({ transform: `translate(${p.x}px, ${p.y}px)` });
    cursor.style.transform = at(to).transform;
    cursor.animate([at(from), at(from), at(mid), at(to)], {
      duration: 1500,
      delay: 380,
      offset: [0, 0.15, 0.6, 1],
      easing: 'ease-in-out',
      fill: 'backwards',
    });
  }

  const [leaves, enters] = EDGES[side];
  caption.textContent = `The pointer leaves the ${leaves} edge of the laptop and enters the ${enters} edge of the monitor.`;
  for (const b of buttons) b.setAttribute('aria-pressed', String(b.dataset.side === side));
}

for (const b of buttons) b.addEventListener('click', () => place(b.dataset.side));
if (svg) {
  place('left');
  requestAnimationFrame(() => requestAnimationFrame(() => svg.classList.add('ready')));
}
