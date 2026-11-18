import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { compose } from './rig.mjs';

const dir = path.dirname(fileURLToPath(import.meta.url));

const BAND = [
  ['bulle', 'caught', 260, 700, 10],
  ['zeste', { eyes: 'squint', brows: 'up', mouth: 'grin', arms: ['rest', 'aim'], legs: ['wide', 'wide'], tilt: 6, dx: -8 }, 200, 560, 74],
  ['paillette', { eyes: 'open', brows: 'up', mouth: 'gasp', arms: ['aim', 'rest'], legs: ['tiptoe', 'tiptoe'], tilt: -6, dx: 8 }, 200, 905, 74],
  ['pave', { eyes: 'squint', brows: 'sus', mouth: 'smirk', arms: ['rest', 'point'], legs: ['wide', 'wide'], tilt: 4, dx: -10 }, 225, 420, 6],
  ['roupi', { eyes: 'lazy', mouth: 'smirk', arms: ['point', 'rest'], legs: ['sit', 'sit'], tilt: -2, dx: 10 }, 225, 1045, 6],
];
const BAND_VIEWBOX = '430 147 842 243';

const SINGLES = [
  ['zeste-accuse', 'zeste', 'accuse', '20 28 166 168'],
  ['paillette-celebrate', 'paillette', 'celebrate', '3 2 196 192'],
  ['paillette-sad', 'paillette', 'sad', '12 42 176 154'],
  ['pave-think', 'pave', 'think', '16 27 181 169'],
];

const num = (v, d = 1) => {
  const s = String(+(+v).toFixed(d));
  return s.replace(/^(-?)0\./, '$1.');
};

function pathData(d) {
  const tokens = d.match(/[a-zA-Z]|-?(?:\d+\.?\d*|\.\d+)(?:e-?\d+)?/g);
  let out = '';
  let prevNum = false;
  for (const t of tokens) {
    if (/[a-zA-Z]/.test(t)) {
      out += t;
      prevNum = false;
    } else {
      const n = num(t);
      if (prevNum && !n.startsWith('-')) out += ' ';
      out += n;
      prevNum = true;
    }
  }
  return out;
}

function matrix(list) {
  const mul = (m, n) => [m[0] * n[0] + m[2] * n[1], m[1] * n[0] + m[3] * n[1], m[0] * n[2] + m[2] * n[3], m[1] * n[2] + m[3] * n[3], m[0] * n[4] + m[2] * n[5] + m[4], m[1] * n[4] + m[3] * n[5] + m[5]];
  let m = [1, 0, 0, 1, 0, 0];
  for (const [, fn, args] of list.matchAll(/(\w+)\(([^)]*)\)/g)) {
    const a = args.trim().split(/[\s,]+/).map(Number);
    if (fn === 'translate') m = mul(m, [1, 0, 0, 1, a[0], a[1] ?? 0]);
    else if (fn === 'scale') m = mul(m, [a[0], 0, 0, a[1] ?? a[0], 0, 0]);
    else if (fn === 'matrix') m = mul(m, a);
    else if (fn === 'rotate') {
      const r = (a[0] * Math.PI) / 180;
      const [cx, cy] = [a[1] ?? 0, a[2] ?? 0];
      m = mul(m, [1, 0, 0, 1, cx, cy]);
      m = mul(m, [Math.cos(r), Math.sin(r), -Math.sin(r), Math.cos(r), 0, 0]);
      m = mul(m, [1, 0, 0, 1, -cx, -cy]);
    } else throw new Error(`transform ${fn}`);
  }
  return m;
}
function transform(list) {
  const m = matrix(list);
  const [a, b, c, d] = m.slice(0, 4).map((v) => num(v, 3));
  const [e, f] = m.slice(4).map((v) => num(v));
  if (a === '1' && b === '0' && c === '0' && d === '1') return e === '0' && f === '0' ? '' : `translate(${e}${f === '0' ? '' : ` ${f}`})`;
  return `matrix(${a} ${b} ${c} ${d} ${e} ${f})`;
}

const colour = (v) => {
  v = v.toLowerCase();
  const m = v.match(/^#([0-9a-f])\1([0-9a-f])\2([0-9a-f])\3$/);
  return m ? `#${m[1]}${m[2]}${m[3]}` : v;
};

function tidyTags(svg) {
  return svg.replace(/<(\w+)((?:\s[\w-]+="[^"]*")*)\s*(\/?)>/g, (all, tag, attrs, self) => {
    const out = [];
    for (const [, k, v] of attrs.matchAll(/\s([\w-]+)="([^"]*)"/g)) {
      if ((k === 'stroke-linecap' || k === 'stroke-linejoin') && v === 'round') continue;
      if (k === 'd') out.push(`d="${pathData(v)}"`);
      else if (k === 'transform') {
        const t = transform(v);
        if (t) out.push(`transform="${t}"`);
      } else if (k === 'fill' || k === 'stroke') out.push(`${k}="${colour(v)}"`);
      else if (/^(cx|cy|r|rx|ry|x|y|width|height|stroke-width)$/.test(k)) out.push(`${k}="${num(v)}"`);
      else if (k === 'opacity') out.push(`opacity="${num(v, 2)}"`);
      else out.push(`${k}="${v}"`);
    }
    return `<${tag}${out.map((a) => ' ' + a).join('')}${self ? '/' : ''}>`;
  });
}

function inner(who, mood, uid) {
  const svg = compose(who, mood, { uid });
  return tidyTags(svg.replace(/^<svg[^>]*>/, '').replace(/<\/svg>$/, ''));
}

function flatten(s) {
  let prev;
  do {
    prev = s;
    s = s.replace(/<g>((?:(?!<\/?g[\s>]).)*)<\/g>/g, '$1').replace(/<g[^>]*><\/g>/g, '');
  } while (s !== prev);
  return s;
}

const root = (viewBox, body) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}" stroke-linecap="round" stroke-linejoin="round">${flatten(body)}</svg>\n`;

const written = [];
const write = (name, svg) => {
  fs.writeFileSync(path.join(dir, `${name}.svg`), svg);
  written.push(name);
};

write(
  'band',
  root(
    BAND_VIEWBOX,
    BAND.map(([who, mood, size, left, bottom]) => {
      const [x, y, s] = [left, 400 - bottom - size, size / 200];
      return `<g transform="${transform(`translate(${x} ${y}) scale(${s})`)}">${inner(who, mood, `b${who[0]}${who[2]}`)}</g>`;
    }).join(''),
  ),
);
for (const [name, who, mood, viewBox] of SINGLES) write(name, root(viewBox, inner(who, mood, name.replace(/[^a-z]/g, '').slice(0, 6))));

console.log(written.map((n) => `${n}.svg ${fs.statSync(path.join(dir, `${n}.svg`)).size} B`).join('\n'));
