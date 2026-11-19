import { type ArtPalette, ROOM_THEMES, SCENE } from '@/lib/tokens';
import type { CuratedId } from '@/lib/utils/room-theme';
import { makeRand } from '@/lib/utils/rand';

export const SCENE_H = 112;
export const SCENE_STAGE = 18;
const BASE = 96;
const TEXT_END = 69;
const TEXT_RIGHT = 184;
const MIN_WIDTH = 240;

type Paint = { fill?: string; grad?: string; op?: number; stroke?: string; sw?: number; flat?: true; tr?: string };
export type ArtShape =
  | (Paint & { el: 'path'; d: string })
  | (Paint & { el: 'circle'; cx: number; cy: number; r: number })
  | (Paint & { el: 'rect'; x: number; y: number; w: number; h: number; rx: number });
type Stop = readonly [offset: number, color: string, alpha: number];
export type ArtGrad =
  | { id: string; kind: 'linear'; x1: number; y1: number; x2: number; y2: number; stops: Stop[] }
  | { id: string; kind: 'radial'; cx: number; cy: number; r: number; stops: Stop[] };
export type Scene = { w: number; h: number; grads: ArtGrad[]; shapes: ArtShape[] };
type Scheme = 'dark' | 'light';
type Draw = (w: number, a: ArtPalette, bg: string, scheme: Scheme) => { grads: ArtGrad[]; shapes: ArtShape[] };

const n = (v: number) => Math.round(v * 10) / 10;
const P = (d: string, o: Paint = {}): ArtShape => ({ el: 'path', d, ...o });
const C = (cx: number, cy: number, r: number, o: Paint = {}): ArtShape => ({ el: 'circle', cx: n(cx), cy: n(cy), r: n(r), ...o });
const R = (x: number, y: number, w: number, h: number, rx = 0, o: Paint = {}): ArtShape => ({ el: 'rect', x: n(x), y: n(y), w: n(w), h: n(h), rx, ...o });
const rng = (seed: number) => makeRand(seed).f;

const disc = (x: number, y: number, r: number) => `M${n(x - r)} ${n(y)}a${n(r)} ${n(r)} 0 1 1 ${n(2 * r)} 0a${n(r)} ${n(r)} 0 1 1 ${n(-2 * r)} 0Z`;
const spark = (x: number, y: number, r: number) =>
  `M${n(x)} ${n(y - r)}Q${n(x)} ${n(y)} ${n(x + r)} ${n(y)}Q${n(x)} ${n(y)} ${n(x)} ${n(y + r)}Q${n(x)} ${n(y)} ${n(x - r)} ${n(y)}Q${n(x)} ${n(y)} ${n(x)} ${n(y - r)}Z`;
const box = (x: number, y: number, w: number, h: number) => `M${n(x)} ${n(y)}h${n(w)}v${n(h)}h${n(-w)}Z`;
const ground = (w: number, y = BASE) => box(-2, y, w + 4, SCENE_H - y + 2);
const gapX = (w: number) => Math.max(190, w - 187);

function scatter(w: number, seed: number) {
  const r = rng(seed);
  const out: [number, number, number][] = [];
  for (let x = 10; x < w - 6; x += 27) out.push([x + r() * 16, 4 + r() * 9, r()]);
  const [x0, x1] = [TEXT_RIGHT, w - 164];
  if (x1 - x0 > 12) for (let y = 18; y < 44; y += 11) out.push([x0 + r() * (x1 - x0), y + r() * 6, r()]);
  return out;
}

function sky(a: ArtPalette): { grad: ArtGrad; shape: ArtShape } {
  return {
    grad: {
      id: 'sky',
      kind: 'linear',
      x1: 0,
      y1: 0,
      x2: 0,
      y2: BASE,
      stops: [
        [0, a.sky[0], 0],
        [TEXT_END / BASE, a.sky[0], a.skyA[0]],
        [1, a.sky[1], a.skyA[1]],
      ],
    },
    shape: R(-2, 0, 4000, BASE + 1, 0, { grad: 'sky' }),
  };
}

const glow = (id: string, cx: number, cy: number, r: number, color: string, alpha: number): ArtGrad => ({
  id,
  kind: 'radial',
  cx: n(cx),
  cy: n(cy),
  r,
  stops: [
    [0, color, alpha],
    [1, color, 0],
  ],
});

const fade = (id: string, y1: number, y2: number, stops: Stop[]): ArtGrad => ({ id, kind: 'linear', x1: 0, y1, x2: 0, y2, stops });

function lumps(w: number, seed: number, y: number, r0: number, r1: number, overlap = 0.62) {
  const q = rng(seed);
  let d = ground(w, y);
  for (let x = -8; x < w + 20; ) {
    const rad = r0 + q() * (r1 - r0);
    d += disc(x, y + rad * 0.3, rad);
    x += rad * (overlap + q() * 0.5) + r0 * 0.5;
  }
  return d;
}

const pyjama: Draw = (w, a, bg, scheme) => {
  const dark = scheme === 'dark';
  const s = sky(a);
  const mx = gapX(w);
  const my = 31;
  let dots = '';
  let sparks = '';
  for (const [x, y, k] of scatter(w, 9)) {
    if (Math.abs(x - mx) < 28 && Math.abs(y - my) < 26) continue;
    if (k > 0.66) sparks += spark(x, y, 3 + k * 3.5);
    else dots += disc(x, y, 0.8 + k * 1.1);
  }
  const moon = `M${n(mx + 3)} ${n(my - 15)}A15 15 0 1 0 ${n(mx + 14)} ${n(my + 7.5)}A12 12 0 0 1 ${n(mx + 3)} ${n(my - 15)}Z`;
  const shade = `M${n(mx - 8)} ${n(my + 7.5)}A15 15 0 0 0 ${n(mx + 14)} ${n(my + 7.5)}A12 12 0 0 1 ${n(mx - 4)} ${n(my + 2)}Z`;
  const z = (x: number, y: number, k: number) => `M${n(x)} ${n(y)}h${n(k)}l${n(-k)} ${n(k)}h${n(k)}`;
  const snore = z(w - 92, 80, 5) + z(w - 76, 71, 6.5) + z(w - 57, 64, 8);
  return {
    grads: [s.grad, glow('halo', mx, my, 46, a.a, dark ? 0.2 : 0.5)],
    shapes: [
      s.shape,
      C(mx, my, 46, { grad: 'halo' }),
      P(dots, { fill: a.c, op: dark ? 0.7 : 0.95 }),
      P(sparks, { fill: a.a }),
      P(moon, { fill: a.a }),
      P(shade, { fill: a.b }),
      P(moon, { stroke: a.ink, sw: 2 }),
      P(lumps(w, 5, 82, 9, 26, 0.5), { fill: a.far ?? bg }),
      P(lumps(w, 12, 91, 7, 19, 0.55), { fill: bg }),
      P(snore, { stroke: a.ink, sw: 5 }),
      P(snore, { stroke: SCENE.white, sw: 2.2 }),
    ],
  };
};

const feu: Draw = (w, a, bg, scheme) => {
  const dark = scheme === 'dark';
  const s = sky(a);
  const fx = w - 108;
  const tx = w - 50;
  const fy = BASE;
  const pine = (x: number, base: number, h: number) => {
    const hw = h * 0.34;
    const top = base - h;
    const p = (dx: number, k: number) => `${n(x + dx * hw)} ${n(top + h * k)}`;
    return `M${p(0, 0)}L${p(0.5, 0.34)}L${p(0.26, 0.34)}L${p(0.78, 0.67)}L${p(0.4, 0.67)}L${p(1, 1)}L${p(-1, 1)}L${p(-0.4, 0.67)}L${p(-0.78, 0.67)}L${p(-0.26, 0.34)}L${p(-0.5, 0.34)}Z`;
  };
  const inClearing = (x: number) => x > fx - 22 && x < tx + 27;
  const forest = (seed: number, base: number, h0: number, h1: number, step: number, front: boolean) => {
    const q = rng(seed);
    let d = ground(w, base - 0.5);
    for (let x = -4; x < w + 12; x += step * (0.75 + q() * 0.8)) {
      let h = h0 + q() * (h1 - h0);
      if (front && inClearing(x)) continue;
      if (x < TEXT_RIGHT) h = Math.min(h, front ? 20 : 26);
      d += pine(x, base, h);
    }
    return d;
  };
  let stars = '';
  for (const [x, y, k] of scatter(w, 23)) if (Math.hypot(x - fx, y - 40) > 24) stars += k > 0.78 ? spark(x, y, 3.6) : disc(x, y, 0.7 + k * 1.1);
  const flame = `M${n(fx)} ${n(fy - 22)}C${n(fx + 2)} ${n(fy - 15)} ${n(fx + 9)} ${n(fy - 13)} ${n(fx + 8)} ${n(fy - 5)}C${n(fx + 7.4)} ${n(fy + 1)} ${n(fx - 7.4)} ${n(fy + 1)} ${n(fx - 8)} ${n(fy - 5)}C${n(fx - 8.6)} ${n(fy - 10)} ${n(fx - 4.6)} ${n(fy - 11)} ${n(fx - 4.6)} ${n(fy - 15.5)}C${n(fx - 2.4)} ${n(fy - 14.5)} ${n(fx - 1.2)} ${n(fy - 18)} ${n(fx)} ${n(fy - 22)}Z`;
  const heart = `M${n(fx + 0.6)} ${n(fy - 12)}C${n(fx + 2.4)} ${n(fy - 8)} ${n(fx + 4.6)} ${n(fy - 7)} ${n(fx + 4.2)} ${n(fy - 3.4)}C${n(fx + 3.8)} ${n(fy)} ${n(fx - 3.4)} ${n(fy)} ${n(fx - 3.8)} ${n(fy - 3.4)}C${n(fx - 4)} ${n(fy - 6)} ${n(fx - 1)} ${n(fy - 7.4)} ${n(fx + 0.6)} ${n(fy - 12)}Z`;
  const embers = disc(fx + 6, fy - 29, 1.5) + disc(fx - 4, fy - 37, 1.2) + disc(fx + 9, fy - 46, 1) + disc(fx + 1, fy - 56, 0.9) + disc(fx + 12, fy - 64, 0.7);
  const tent = `M${n(tx)} ${n(fy - 27)}L${n(tx + 23)} ${n(fy + 0.5)}L${n(tx - 23)} ${n(fy + 0.5)}Z`;
  const side = `M${n(tx)} ${n(fy - 27)}L${n(tx + 23)} ${n(fy + 0.5)}L${n(tx + 8)} ${n(fy + 0.5)}Z`;
  const door = `M${n(tx)} ${n(fy - 16)}L${n(tx + 7.6)} ${n(fy + 0.5)}L${n(tx - 7.6)} ${n(fy + 0.5)}Z`;
  const logs = `M${n(fx - 10)} ${n(fy - 0.5)}L${n(fx + 10)} ${n(fy - 4)}M${n(fx - 10)} ${n(fy - 4)}L${n(fx + 10)} ${n(fy - 0.5)}`;
  return {
    grads: [s.grad, glow('fire', fx, fy - 8, 62, a.b, dark ? 0.55 : 0.5)],
    shapes: [
      s.shape,
      P(stars, { fill: SCENE.white, op: dark ? 0.7 : 0.95 }),
      P(forest(3, BASE - 3, 26, 44, 17, false), { fill: a.far ?? bg }),
      C(fx, fy - 8, 62, { grad: 'fire' }),
      P(forest(8, BASE, 15, 30, 14, true), { fill: bg }),
      P(tent, { fill: a.c, stroke: a.ink, sw: 2 }),
      P(side, { fill: a.ink, op: 0.16 }),
      P(door, { fill: a.a, stroke: a.ink, sw: 1.8 }),
      P(`M${n(tx)} ${n(fy - 27)}v-5`, { stroke: a.ink, sw: 2 }),
      P(logs, { stroke: a.ink, sw: 5.6 }),
      P(logs, { stroke: SCENE.log[scheme], sw: 2.6 }),
      P(flame, { fill: a.b, stroke: a.ink, sw: 2 }),
      P(heart, { fill: a.a }),
      P(embers, { fill: a.a, op: 0.95 }),
    ],
  };
};

const karaoke: Draw = (w, a, bg, scheme) => {
  const dark = scheme === 'dark';
  const s = sky(a);
  const bx = gapX(w);
  const by = 27;
  const br = 13;
  const q = rng(77);
  let bars = '';
  let bars2 = '';
  let i = 0;
  const mic = w - 78;
  for (let x = 3; x < w - 4; x += 11, i++) {
    const wave = 0.5 + 0.3 * Math.sin(i * 0.5 + 0.6) + 0.2 * Math.sin(i * 1.31 + 2);
    let h = 7 + (0.2 + 0.8 * wave) * (0.55 + 0.45 * q()) * 27;
    if (x < TEXT_RIGHT) h = Math.min(h, 19);
    if (Math.abs(x + 3.5 - mic) < 26) h = Math.min(h, 9);
    const bar = `M${n(x)} ${n(BASE + 8)}v${n(-h - 8)}a3.5 3.5 0 0 1 7 0v${n(h + 8)}Z`;
    if (i % 3 === 2) bars2 += bar;
    else bars += bar;
  }
  const tiles =
    `M${n(bx - br)} ${n(by)}h${n(2 * br)}M${n(bx - br * 0.87)} ${n(by - br * 0.5)}h${n(br * 1.74)}M${n(bx - br * 0.87)} ${n(by + br * 0.5)}h${n(br * 1.74)}` +
    `M${n(bx)} ${n(by - br)}v${n(2 * br)}M${n(bx)} ${n(by - br)}a${n(br * 0.5)} ${n(br)} 0 0 0 0 ${n(2 * br)}M${n(bx)} ${n(by - br)}a${n(br * 0.5)} ${n(br)} 0 0 1 0 ${n(2 * br)}`;
  const beam = (x0: number, x1: number, x2: number) => `M${n(x0 - 6)} -2L${n(x0 + 6)} -2L${n(x2)} ${BASE + 6}L${n(x1)} ${BASE + 6}Z`;
  const tilt = `translate(${n(mic)} 76) rotate(32) scale(0.9)`;
  const note = (x: number, y: number, k: number) => `M${n(x + 2.6 * k)} ${n(y)}v${n(-9 * k)}q${n(3 * k)} ${n(0.5 * k)} ${n(4.4 * k)} ${n(4 * k)}`;
  const heads = (pts: [number, number, number][]) => pts.map(([x, y, k]) => disc(x, y, 2.7 * k)).join('');
  const notes: [number, number, number][] = [
    [mic + 28, 80, 1],
    [mic + 43, 73, 0.8],
  ];
  const stems = notes.map(([x, y, k]) => note(x, y, k)).join('');
  const [ballHi, ballMid, ballLo] = SCENE.mirrorBall[scheme];
  const soft = dark ? 0.55 : 0.6;
  return {
    grads: [
      s.grad,
      fade('beamA', 0, BASE, [
        [0, a.a, dark ? 0.36 : 0.42],
        [1, a.a, 0.03],
      ]),
      fade('beamB', 0, BASE, [
        [0, a.b, dark ? 0.36 : 0.5],
        [1, a.b, 0.03],
      ]),
      {
        id: 'ball',
        kind: 'linear',
        x1: bx - br,
        y1: by - br,
        x2: bx + br,
        y2: by + br,
        stops: [
          [0, ballHi, 1],
          [0.5, ballMid, 1],
          [1, ballLo, 1],
        ],
      },
      fade('bars', BASE - 34, BASE + 6, [
        [0, a.a, 1],
        [0.55, a.a, soft],
        [1, a.a, 0],
      ]),
      fade('bars2', BASE - 34, BASE + 6, [
        [0, a.b, 1],
        [0.55, a.b, soft],
        [1, a.b, 0],
      ]),
    ],
    shapes: [
      s.shape,
      P(beam(w * 0.06, w * 0.26, w * 0.6), { grad: 'beamA' }),
      P(beam(w * 0.95, w * 0.44, w * 0.8), { grad: 'beamB' }),
      P(spark(bx - 23, by + 10, 5) + spark(bx + 22, by - 10, 6.5) + spark(bx + 18, by + 18, 3.5) + spark(bx - 17, by - 14, 3), { fill: SCENE.white, op: 0.95 }),
      P(`M${n(bx)} -2V${n(by - br)}`, { stroke: SCENE.mirrorCord[scheme], sw: 1.6 }),
      C(bx, by, br, { grad: 'ball', stroke: a.ink, sw: 2 }),
      P(tiles, { stroke: a.ink, sw: 1, op: 0.5 }),
      P(box(bx - 6.6, by - 6.4, 4.4, 4.4), { fill: SCENE.white, op: 0.9 }),
      P(bars, { grad: 'bars' }),
      P(bars2, { grad: 'bars2' }),
      P('M-3.6 8L-2.6 30a2.6 2.6 0 0 0 5.2 0L3.6 8Z', { fill: a.b, stroke: a.ink, sw: 2, tr: tilt }),
      R(-5, 5.5, 10, 4.6, 1.6, { fill: a.c, stroke: a.ink, sw: 1.8, tr: tilt }),
      C(0, -2, 9, { fill: SCENE.micHead[scheme], stroke: a.ink, sw: 2, tr: tilt }),
      P('M-8.4 -4.4h16.8M-7.6 1.6h15.2M-3 -10.4q-2.6 8.4 0 16.8M3 -10.4q2.6 8.4 0 16.8', { stroke: a.ink, sw: 1, op: 0.45, tr: tilt }),
      P(stems, { stroke: a.ink, sw: 4.6 }),
      P(heads(notes), { fill: a.c, stroke: a.ink, sw: 1.6 }),
      P(stems, { stroke: a.c, sw: 2 }),
    ],
  };
};

function pixels(grid: string[], x: number, y: number, u: number, ch = 'X') {
  let d = '';
  grid.forEach((row, j) => {
    for (let i = 0; i < row.length; i++) {
      if (row[i] !== ch) continue;
      let k = i;
      while (k + 1 < row.length && row[k + 1] === ch) k++;
      d += box(x + i * u, y + j * u, (k - i + 1) * u, u);
      i = k;
    }
  });
  return d;
}

const HEART = ['.XX.XX.', 'XXXXXXX', 'XXXXXXX', '.XXXXX.', '..XXX..', '...X...'];
const COIN = ['.XXX.', 'XXXXX', 'XXXXX', 'XXXXX', 'XXXXX', '.XXX.'];
const SHINE = ['.....', '.X...', '.X...', '.X...', '.....', '.....'];
const BEAN = ['...XX.....', '..XXXXXX..', '.XXXXXXXX.', 'XXoXXXXoXX', 'XXoXXXXoXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '.XXXXXXXX.', '.XX....XX.'];

const arcade: Draw = (w, a, bg, scheme) => {
  const dark = scheme === 'dark';
  const s = sky(a);
  const q = rng(64);
  let land = ground(w, BASE);
  let edge = '';
  let x = -2;
  let level = 1;
  const hero = w - 126;
  let heroTop = BASE;
  let flagAt: [number, number] = [w - 30, BASE];
  while (x < w + 2) {
    const len = 26 + Math.floor(q() * 3) * 13;
    let next = Math.max(0, Math.min(3, level + (q() < 0.5 ? -1 : 1)));
    if (x < TEXT_RIGHT) next = Math.min(next, 2);
    if (hero >= x - 16 && hero < x + len + 16) next = 0;
    level = next;
    const top = BASE - level * 8;
    land += box(x, top, len + 0.5, BASE - top + 1);
    edge += box(x, top, len + 0.5, 3);
    if (hero >= x && hero < x + len) heroTop = top;
    if (w - 26 >= x && w - 26 < x + len) flagAt = [w - 26, top];
    x += len;
  }
  let stars = '';
  for (const [sx, sy, k] of scatter(w, 31)) if (sy < 16 && sx < w - 70) stars += box(Math.round(sx), Math.round(sy), k > 0.7 ? 3 : 2, k > 0.7 ? 3 : 2);
  const cx = gapX(w) - 26;
  const coins: [number, number][] = [
    [cx, 35],
    [cx + 22, 22],
    [cx + 44, 35],
  ];
  const hx = w - 62;
  const [fgx, fgy] = flagAt;
  const bean = pixels(BEAN, hero - 12.5, heroTop - 22.5, 2.5);
  return {
    grads: [s.grad],
    shapes: [
      s.shape,
      P(stars, { fill: SCENE.white, op: dark ? 0.7 : 0.95 }),
      P(coins.map(([x0, y0]) => pixels(COIN, x0, y0, 2.6)).join(''), { fill: a.b }),
      P(coins.map(([x0, y0]) => pixels(SHINE, x0, y0, 2.6)).join(''), { fill: SCENE.white, op: 0.85 }),
      P(pixels(HEART, hx, 6, 2) + pixels(HEART, hx + 17, 6, 2), { fill: a.c }),
      P(pixels(HEART, hx + 34, 6, 2), { fill: a.c, op: 0.3 }),
      P(land, { fill: bg }),
      P(edge, { fill: a.a }),
      P(box(fgx, fgy - 22, 2.5, 22), { fill: dark ? SCENE.white : a.ink, op: dark ? 0.85 : 0.8 }),
      P(box(fgx + 2.5, fgy - 22, 10, 2.5) + box(fgx + 2.5, fgy - 19.5, 7.5, 2.5) + box(fgx + 2.5, fgy - 17, 5, 2.5), { fill: a.c }),
      ...(dark ? [] : [P(bean, { fill: SCENE.bean, stroke: a.ink, sw: 1.6 })]),
      P(bean, { fill: SCENE.bean }),
      P(pixels(BEAN, hero - 12.5, heroTop - 22.5, 2.5, 'o'), { fill: a.ink }),
    ],
  };
};

const piscine: Draw = (w, a, bg, scheme) => {
  const dark = scheme === 'dark';
  const s = sky(a);
  const waves = (y: number, len: number, amp: number, shift: number) => {
    let d = `M${n(-len + shift)} ${n(y)}`;
    for (let x = -len + shift; x < w + len; x += len) d += `q${n(len / 4)} ${n(-amp)} ${n(len / 2)} 0t${n(len / 2)} 0`;
    return d + `V${SCENE_H + 2}H${n(-len + shift)}Z`;
  };
  const rx = w - 118;
  const ry = 82;
  const stripes = [25, 115, 205, 295]
    .map((deg) => {
      const [p, q] = [(deg * Math.PI) / 180, ((deg + 40) * Math.PI) / 180];
      return `M${n(rx + 11.5 * Math.cos(p))} ${n(ry + 11.5 * Math.sin(p))}A11.5 11.5 0 0 1 ${n(rx + 11.5 * Math.cos(q))} ${n(ry + 11.5 * Math.sin(q))}`;
    })
    .join('');
  const dx = w - 58;
  const dy = 88;
  const duck = `M${n(dx - 14)} ${n(dy - 4)}C${n(dx - 14)} ${n(dy + 8)} ${n(dx + 13)} ${n(dy + 8)} ${n(dx + 13)} ${n(dy - 3)}C${n(dx + 13)} ${n(dy - 6)} ${n(dx + 11)} ${n(dy - 7.5)} ${n(dx + 8.5)} ${n(dy - 7.5)}C${n(dx + 12)} ${n(dy - 11)} ${n(dx + 11)} ${n(dy - 19.5)} ${n(dx + 3.5)} ${n(dy - 19.5)}C${n(dx - 3.5)} ${n(dy - 19.5)} ${n(dx - 5)} ${n(dy - 12)} ${n(dx - 2)} ${n(dy - 8)}C${n(dx - 7)} ${n(dy - 6)} ${n(dx - 10.5)} ${n(dy - 10)} ${n(dx - 14)} ${n(dy - 4)}Z`;
  const bx = gapX(w);
  const by = 31;
  let stars = '';
  for (const [x, y, k] of scatter(w, 17)) if (Math.abs(x - bx) > 26 && k > 0.45) stars += spark(x, y, 2 + k * 3);
  const glint = (x: number, y: number) => `M${n(x)} ${n(y)}q4 -3 8 0`;
  return {
    grads: [s.grad, glow('sun', bx, by, 48, SCENE.white, dark ? 0.12 : 0.6)],
    shapes: [
      s.shape,
      C(bx, by, 48, { grad: 'sun' }),
      P(stars, { fill: SCENE.white, op: dark ? 0.55 : 0.95 }),
      C(bx, by, 12, { fill: SCENE.white }),
      P(`M${n(bx)} ${n(by)}L${n(bx - 10.4)} ${n(by - 6)}A12 12 0 0 1 ${n(bx + 1)} ${n(by - 12)}Z`, { fill: a.b }),
      P(`M${n(bx)} ${n(by)}L${n(bx + 11.8)} ${n(by - 2.2)}A12 12 0 0 1 ${n(bx + 4.4)} ${n(by + 11.2)}Z`, { fill: a.a }),
      P(`M${n(bx)} ${n(by)}L${n(bx - 5)} ${n(by + 10.9)}A12 12 0 0 1 ${n(bx - 11.9)} ${n(by + 1.6)}Z`, { fill: SCENE.beachBall }),
      C(bx, by, 12, { stroke: a.ink, sw: 2 }),
      C(bx, by, 2.4, { fill: SCENE.white, stroke: a.ink, sw: 1.3 }),
      P(waves(84, 34, 6, 8), { fill: a.far ?? bg }),
      P(glint(w * 0.18, 84) + glint(w * 0.44, 86) + glint(w * 0.9, 85), { stroke: SCENE.white, sw: 1.6, op: dark ? 0.4 : 0.9 }),
      C(rx, ry, 11.5, { stroke: a.ink, sw: 11.5 }),
      C(rx, ry, 11.5, { stroke: a.b, sw: 7.5 }),
      P(stripes, { stroke: SCENE.white, sw: 7.5, flat: true }),
      P(duck, { fill: a.a, stroke: a.ink, sw: 2 }),
      P(`M${n(dx + 10)} ${n(dy - 15.5)}l7 1.8l-6.6 3.2Z`, { fill: a.b, stroke: a.ink, sw: 1.6 }),
      C(dx + 5.4, dy - 14.6, 1.5, { fill: a.ink }),
      P(`M${n(dx - 7)} ${n(dy - 2)}q4.6 4.6 10 0`, { stroke: a.ink, sw: 1.6 }),
      P(waves(92, 40, 6.5, 0), { fill: bg }),
    ],
  };
};

const apero: Draw = (w, a, bg, scheme) => {
  const dark = scheme === 'dark';
  const s = sky(a);
  const sx = w - 104;
  const sr = 34;
  const posts = [-12, w * 0.31, w * 0.66, w + 12];
  let wire = '';
  const bulbs: [number, number][] = [];
  for (let i = 0; i < posts.length - 1; i++) {
    const [x0, x1] = [posts[i], posts[i + 1]];
    const sag = 8 + (i % 2) * 3;
    wire += `M${n(x0)} 2Q${n((x0 + x1) / 2)} ${n(2 + 2 * sag)} ${n(x1)} 2`;
    const count = Math.max(3, Math.round((x1 - x0) / 25));
    for (let k = 1; k < count; k++) {
      const u = k / count;
      bulbs.push([(1 - u) * (1 - u) * x0 + 2 * (1 - u) * u * ((x0 + x1) / 2) + u * u * x1, 2 + 2 * (1 - u) * u * 2 * sag + 4.8]);
    }
  }
  const colours = SCENE.lightsBulbs[scheme];
  const byColour = colours.map((_, c) => bulbs.filter((_, i) => i % colours.length === c));
  const parasol = (x: number, hw: number, h: number) => `M${n(x - hw)} ${n(BASE - h + 9)}Q${n(x)} ${n(BASE - h - 9)} ${n(x + hw)} ${n(BASE - h + 9)}Z` + box(x - 1, BASE - h + 8, 2, h - 7);
  const tx = w - 152;
  const bottle = `M${n(tx)} ${n(BASE + 1)}v-19q0 -4 3 -5.6v-8.4h5v8.4q3 1.6 3 5.6v19Z`;
  const gx = tx + 33;
  const glass = `M${n(gx - 8)} ${n(BASE - 25)}h16q0 11 -6.4 13v9.6h5.4v3h-14v-3h5.4v-9.6q-6.4 -2 -6.4 -13Z`;
  const cx = w - 40;
  const cy = BASE + 1;
  const bowl = `M${n(cx - 12)} ${n(cy - 30)}h24c1.6 9 -2.8 17 -9.4 18.2v8h6v3h-17.2v-3h6v-8c-6.6 -1.2 -11 -9.2 -9.4 -18.2Z`;
  const drink = `M${n(cx - 10.4)} ${n(cy - 23)}h20.8c0 6 -4 10.4 -10.4 10.4s-10.4 -4.4 -10.4 -10.4Z`;
  const straw = `M${n(cx + 3)} ${n(cy - 20)}L${n(cx + 11.5)} ${n(cy - 35.5)}`;
  const onItsFoot = `translate(${n(cx * 0.12)} ${n(cy * 0.12)}) scale(0.88)`;
  return {
    grads: [
      s.grad,
      glow('halo', sx, BASE, 66, a.a, dark ? 0.5 : 0.8),
      fade('sun', BASE + 7 - sr, BASE, [
        [0, a.a, 1],
        [0.55, a.b, 1],
        [1, a.c, 1],
      ]),
    ],
    shapes: [
      s.shape,
      C(sx, BASE, 66, { grad: 'halo' }),
      C(sx, BASE + 7, sr, { grad: 'sun' }),
      R(sx - 14, BASE - 9, 44, 3.4, 1.7, { fill: a.sky[0], op: 0.9 }),
      R(sx - 44, BASE - 18, 36, 3, 1.5, { fill: a.sky[0], op: 0.8 }),
      P(wire, { stroke: SCENE.lightsWire[scheme], sw: 1.4 }),
      ...(dark ? [P(bulbs.map(([x, y]) => disc(x, y, 8)).join(''), { fill: a.a, op: 0.12 })] : []),
      ...byColour.map((list, c) => P(list.map(([x, y]) => disc(x, y, 3.4)).join(''), { fill: colours[c] })),
      P(ground(w, BASE) + parasol(52, 22, 21) + parasol(118, 17, 16) + bottle + glass, { fill: bg }),
      P(bowl, { fill: SCENE.white, op: dark ? 0.14 : 0.75, tr: onItsFoot }),
      P(drink, { fill: a.b, tr: onItsFoot }),
      P(box(cx - 5.4, cy - 21.4, 4.8, 4.8) + box(cx + 1, cy - 19.4, 4.4, 4.4), { fill: SCENE.white, op: 0.5, tr: onItsFoot }),
      P(straw, { stroke: a.ink, sw: 4.4, tr: onItsFoot }),
      P(straw, { stroke: a.c, sw: 2, tr: onItsFoot }),
      P(bowl, { stroke: a.ink, sw: 2, tr: onItsFoot }),
      C(cx - 11.5, cy - 30, 6, { fill: a.a, stroke: a.ink, sw: 1.8, tr: onItsFoot }),
      P(`M${n(cx - 11.5)} ${n(cy - 34.6)}v9.2M${n(cx - 16.1)} ${n(cy - 30)}h9.2`, { stroke: a.b, sw: 1.3, tr: onItsFoot }),
    ],
  };
};

const DRAW: Record<CuratedId, Draw> = { apero, feu, karaoke, pyjama, arcade, piscine };

const cache = new Map<string, Scene>();

export function scene(id: CuratedId, dark: boolean, width: number): Scene {
  const w = Math.max(MIN_WIDTH, Math.round(width));
  const scheme: Scheme = dark ? 'dark' : 'light';
  const key = `${id}|${scheme}|${w}`;
  let hit = cache.get(key);
  if (!hit) {
    const theme = ROOM_THEMES[id];
    hit = { w, h: SCENE_H, ...DRAW[id](w, theme.art[scheme], theme[scheme].bg, scheme) };
    cache.set(key, hit);
  }
  return hit;
}
