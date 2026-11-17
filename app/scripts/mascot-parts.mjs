// Builds the mascots ("la bande", Petons rig) into the one JSON read by the app (src/lib/mascot-parts.json, drawn by
// components/mascot.tsx) and the server (server/assets/mascot.json, byte-identical, for share cards later).
// Run from app/: `node scripts/mascot-parts.mjs` (`--check` only verifies both files are up to date).
//
// Five friends, one shared part library. Every character = a BODY (silhouette + anchors) + PARTS (eyes, brows, mouth,
// arms, legs, props) picked by a MOOD (+ per-character QUIRKS). Parts are drawn once in local coordinates:
//   - face parts around the face centre (0,0), eyes at x = ±20, mouth at y ≈ 16..33;
//   - arm and leg parts for the RIGHT socket (+x = outward); the left socket draws the same part mirrored;
//   - `prop` props around the body's prop anchor, `face` props in face space, `abs` props in the 200×200 box.
// Shapes use the avatar format (lib/avatar.ts `Shape`): strokes are round-capped. Colours may be tokens resolved per
// character at draw time: ink, body, shade, cheek, white, acc2. `hd: 1` = detail dropped at small sizes (LOD).
// Every transform of the original rig (rotated feet, sitting soles) is baked into the geometry: no transform attributes.
import { readFileSync, writeFileSync } from 'node:fs';

const INK = '#121216';
const GROUND = 187; // feet rest here; tilt / squash pivot

// ---------- numbers ----------
const r1 = (n) => Math.round(n * 10) / 10 || 0;
/** Round every number of a path to 0.1 (the box is 200 units: 0.05 at most, under 0.05 px at 160 px). */
const rp = (d) =>
  d
    .replace(/-?\d*\.?\d+(e-?\d+)?/g, (n) => String(r1(+n)))
    .replace(/\s+/g, ' ')
    .replace(/ ?([A-Za-z]) ?/g, '$1')
    .replace(/ -/g, '-')
    .trim();

// ---------- tiny element helpers ----------
const P = (d, o = {}) => ({ el: 'path', d, ...o });
const C = (cx, cy, r, o = {}) => ({ el: 'circle', cx, cy, r, ...o });
const E = (cx, cy, rx, ry, o = {}) => ({ el: 'ellipse', cx, cy, rx, ry, ...o });
const R = (x, y, w, h, rx, o = {}) => ({ el: 'rect', x, y, w, h, rx, ...o });
const hd = (s) => ({ ...s, hd: 1 });
const LW = 5.5; // face line weight
const line = (d, sw = LW) => P(d, { stroke: 'ink', sw });
/** An outlined tube: ink stroke, then a narrower stroke in `inner` on top (arms, legs, stems, letters). */
const tube = (d, w = 22, inner = 'body', outline = 5.5) => [P(d, { stroke: 'ink', sw: w }), P(d, { stroke: inner, sw: r1(w - 2 * outline) })];

/** A rotated ellipse as a path (two arcs), so no transform is needed. */
function rotEllipse(cx, cy, rx, ry, deg, o) {
  const a = (deg * Math.PI) / 180;
  const [dx, dy] = [rx * Math.cos(a), rx * Math.sin(a)];
  return P(`M${cx + dx} ${cy + dy}A${rx} ${ry} ${deg} 1 1 ${cx - dx} ${cy - dy}A${rx} ${ry} ${deg} 1 1 ${cx + dx} ${cy + dy}Z`, o);
}
/** Applies translate(tx ty) rotate(deg) to an absolute M/L/C/Q/Z path. */
function movePath(d, tx, ty, deg) {
  const a = (deg * Math.PI) / 180;
  const [c, s] = [Math.cos(a), Math.sin(a)];
  return d.replace(/(-?[\d.]+)[ ,]+(-?[\d.]+)/g, (_, x, y) => `${+x * c - +y * s + tx} ${+x * s + +y * c + ty}`);
}
const movePoint = (x, y, tx, ty, deg) => {
  const a = (deg * Math.PI) / 180;
  return [x * Math.cos(a) - y * Math.sin(a) + tx, x * Math.sin(a) + y * Math.cos(a) + ty];
};

/**
 * Joint: the inner colour of a limb tube redrawn OVER the body outline where the limb leaves the body, so limbs grow out
 * of one continuous silhouette (no ink gap: on dark backgrounds that made hands and feet look detached).
 * Supports 'M x y Q x y x y' and 'M x y L x y'. axis/lo/hi = the stretch of the curve to redraw (local coords).
 */
function joint(d, w, inner, outline, axis, lo, hi) {
  const n = d.match(/-?[\d.]+/g).map(Number);
  const [p0, p1, p2] = n.length === 6 ? [[n[0], n[1]], [n[2], n[3]], [n[4], n[5]]] : [[n[0], n[1]], null, [n[2], n[3]]];
  const c = p1 ?? [(p0[0] + p2[0]) / 2, (p0[1] + p2[1]) / 2];
  const at = (t) => [0, 1].map((i) => (1 - t) ** 2 * p0[i] + 2 * (1 - t) * t * c[i] + t * t * p2[i]);
  const k = axis === 'x' ? 0 : 1;
  const ts = [];
  for (let t = 0; t <= 1.0001; t += 0.01) if (at(t)[k] >= lo && at(t)[k] <= hi) ts.push(t);
  if (!ts.length) return [];
  const [t0, t1] = [ts[0], ts[ts.length - 1]];
  const a = at(t0);
  const b = at(t1);
  const m = [0, 1].map((i) => (1 - t0) * (1 - t1) * p0[i] + ((1 - t0) * t1 + t0 * (1 - t1)) * c[i] + t0 * t1 * p2[i]);
  return [P(`M${a[0]} ${a[1]}Q${m[0]} ${m[1]} ${b[0]} ${b[1]}`, { stroke: inner, sw: r1(w - 2 * outline) })];
}
const armJoint = (d) => joint(d, 22, 'body', 5.5, 'x', -4, 9);
const legJoint = (d, w = 20) => joint(d, w, 'shade', 5.5, 'y', 3, 14);
const mitt = (x, y, r = 12.5) => C(x, y, r, { fill: 'body', stroke: 'ink', sw: 5.5 });
const foot = (x, y, rx = 14, ry = 8.5, rot = 0) =>
  rot ? rotEllipse(x, y, rx, ry, rot, { fill: 'shade', stroke: 'ink', sw: 5.5 }) : E(x, y, rx, ry, { fill: 'shade', stroke: 'ink', sw: 5.5 });
const star = (cx, cy, r, fill, sw = 3.6) =>
  P(
    `M${cx} ${cy - r}Q${cx + r * 0.18} ${cy - r * 0.18} ${cx + r} ${cy}Q${cx + r * 0.18} ${cy + r * 0.18} ${cx} ${cy + r}Q${cx - r * 0.18} ${cy + r * 0.18} ${cx - r} ${cy}Q${cx - r * 0.18} ${cy - r * 0.18} ${cx} ${cy - r}Z`,
    { fill, stroke: 'ink', sw },
  );
const drop = (x, y, h, fill = '#BDEBFF') =>
  P(`M${x} ${y}C${x + h * 0.36} ${y + h * 0.55} ${x + h * 0.45} ${y + h * 0.82} ${x} ${y + h}C${x - h * 0.45} ${y + h * 0.82} ${x - h * 0.36} ${y + h * 0.55} ${x} ${y}Z`, {
    fill,
    stroke: 'ink',
    sw: 4,
  });
/** Sole seen from the front when sitting: an egg, wide at the toes, splayed outward (toe pads are detail). */
const sole = (x, y, rot) => [
  P(movePath('M0 -15C9 -15 12.5 -7 11.5 1C10.5 9 6 13 0 13C-6 13 -10.5 9 -11.5 1C-12.5 -7 -9 -15 0 -15Z', x, y, rot), { fill: 'shade', stroke: 'ink', sw: 5.5 }),
  hd(P(movePath('M0 -1C4.5 -1 6 2 5.5 5C5 8 3 9 0 9C-3 9 -5 8 -5.5 5C-6 2 -4.5 -1 0 -1Z', x, y, rot), { fill: 'body' })),
  ...[
    [-5, -8.5],
    [0, -10],
    [5, -8.5],
  ].map(([px, py]) => hd(C(...movePoint(px, py, x, y, rot), 2.4, { fill: 'body' }))),
];
const EX = 20;
const both = (f) => [...f(-EX, -1), ...f(EX, 1)];

// ---------- BODIES: silhouette + anchors. All sit on one baseline: body bottom ≈ 166, hips y = 158, feet on GROUND. ----------
const BODIES = {
  zeste: {
    color: '#C8FF3D',
    shade: '#9FD61A',
    cheek: '#FF8FA8',
    acc2: '#7BB300',
    d: 'M100 58C132 58 150 80 152 108C154 134 156 152 146 160C138 166 120 166 100 166C80 166 62 166 54 160C44 152 46 134 48 108C50 80 68 58 100 58Z',
    gloss: 'M62 106Q63 86 76 74',
    face: [100, 106, 1],
    arms: [
      [50, 128],
      [150, 128],
    ],
    legs: [
      [81, 158],
      [119, 158],
    ],
    prop: [148, 50],
    shadow: 50,
    back: tube('M100 62C98 50 101 41 110 37C119 33 128 39 126 48C124 55 117 56 113 52', 15, 'acc2', 4.8),
    front: [],
    faceUnder: [],
    faceExtras: [],
  },
  bulle: {
    color: '#5AC8FA',
    shade: '#2FA6E0',
    cheek: '#FF8FC0',
    acc2: '#2FA6E0',
    d: 'M100 68C140 68 168 92 168 121C168 151 142 166 100 166C58 166 32 151 32 121C32 92 60 68 100 68Z',
    gloss: 'M48 114Q50 94 66 82',
    face: [100, 118, 1],
    arms: [
      [36, 134],
      [164, 134],
    ],
    legs: [
      [79, 158],
      [121, 158],
    ],
    prop: [156, 62],
    shadow: 58,
    back: [P('M86 74C82 62 87 52 97 50C94 57 96 63 101 69C102 59 109 52 119 53C113 58 112 65 115 74Z', { fill: 'body', stroke: 'ink', sw: 6.5 })],
    front: [],
    faceUnder: [],
    faceExtras: [],
  },
  pave: {
    color: '#FFB547',
    shade: '#F08F1F',
    cheek: '#FF7A5C',
    acc2: '#F08F1F',
    d: 'M68 72H132C155 72 162 82 163 102L164 142C164 160 156 166 136 166H64C44 166 36 160 36 142L37 102C38 82 45 72 68 72Z',
    gloss: 'M50 124V104Q50 88 62 84',
    face: [100, 116, 1],
    arms: [
      [39, 138],
      [161, 138],
    ],
    legs: [
      [77, 158],
      [123, 158],
    ],
    prop: [158, 56],
    shadow: 60,
    back: tube('M94 74C88 64 94 54 106 54C102 59 104 64 110 68', 14, 'body', 4.8),
    front: [],
    // round glasses: lenses under the eyes, frames over them
    faceUnder: [hd(R(-36, -16, 32, 30, 12, { fill: 'white', op: 0.5 })), hd(R(4, -16, 32, 30, 12, { fill: 'white', op: 0.5 }))],
    faceExtras: [
      hd(P('M-36 -4L-43 -7M36 -4L43 -7', { stroke: 'ink', sw: 5 })),
      P('M-5 -4Q0 -9 5 -4', { stroke: 'ink', sw: 5 }),
      R(-36, -16, 32, 30, 12, { stroke: 'ink', sw: 5 }),
      R(4, -16, 32, 30, 12, { stroke: 'ink', sw: 5 }),
    ],
  },
  paillette: {
    color: '#FF6DB5',
    shade: '#E8479A',
    cheek: '#FF3F8C',
    acc2: '#E8479A',
    d: 'M126 34C120 50 100 62 82 74C58 90 38 108 38 132C38 154 62 166 100 166C138 166 162 154 162 132C162 108 151 92 139 79C129 67 121 52 126 34Z',
    gloss: 'M54 126Q56 106 70 94',
    face: [100, 126, 1],
    arms: [
      [41, 142],
      [159, 142],
    ],
    legs: [
      [80, 158],
      [120, 158],
    ],
    prop: [158, 64],
    shadow: 56,
    back: [],
    front: [],
    faceUnder: [],
    // lashes + beauty mark: the drama queen
    faceExtras: [P('M-26 -10L-30 -17M-29.5 -4.5L-36.5 -8M26 -10L30 -17M29.5 -4.5L36.5 -8', { stroke: 'ink', sw: 4.4 }), hd(C(31, 22, 2.8, { fill: 'ink' }))],
  },
  roupi: {
    color: '#B48CFF',
    shade: '#9367F0',
    cheek: '#F08CE0',
    acc2: '#5EF2B0',
    d: 'M26 150C26 112 58 86 100 86C142 86 174 112 174 150C174 160 168 166 158 166H42C32 166 26 160 26 150Z',
    gloss: 'M40 140Q42 120 56 108',
    face: [100, 133, 0.86],
    arms: [
      [30, 142],
      [170, 142],
    ],
    legs: [
      [73, 158],
      [127, 158],
    ],
    prop: [46, 62],
    shadow: 66,
    back: [],
    // menthe nightcap
    front: [
      P('M60 100C60 76 76 54 100 46C122 38 148 38 162 54C170 63 168 76 162 82C158 70 148 64 138 68C141 78 142 90 141 100C120 90 80 90 60 100Z', {
        fill: 'acc2',
        stroke: 'ink',
        sw: 6.5,
      }),
      ...tube('M56 102Q100 82 145 102', 20, 'white', 5.5),
      C(164, 84, 11.5, { fill: 'white', stroke: 'ink', sw: 5.5 }),
    ],
    faceUnder: [],
    faceExtras: [],
  },
};

// ---------- PARTS ----------
// Eye highlights: the big one stays, the small second one is detail.
const hi = (x, y = -4.8, r = 4.4) => [C(x - 3.2, y, r, { fill: 'white' }), hd(C(x + 3.8, y + 9.4, 1.9, { fill: 'white' }))];
const arm = (d, rest) => ({ z: 'back', els: [...tube(d), ...rest], joint: armJoint(d) });
const leg = (d, rest, lift = 0, w = 20) => ({ z: 'back', lift, els: [...tube(d, w, 'shade'), ...rest], joint: legJoint(d, w) });

const PARTS = {
  eyes: {
    open: both((x) => [E(x, 0, 10, 12.5, { fill: 'ink' }), ...hi(x)]),
    // half-lidded (bored / smug / sleepy-happy)
    lazy: both((x) => [P(`M${x - 9.85} -1A10 12.5 0 0 0 ${x + 9.85} -1Z`, { fill: 'ink' }), C(x - 3, 3.5, 2.6, { fill: 'white' }), line(`M${x - 12} -2L${x + 12} -2`, 6)]),
    // suspicious: lid slanted down toward the nose
    squint: both((x, s) => [
      P(`M${x - 9.85} 2A10 12.5 0 0 0 ${x + 9.85} 2Z`, { fill: 'ink' }),
      C(x - 3 * s, 6, 2.4, { fill: 'white' }),
      line(s < 0 ? `M${x - 12} -3L${x + 11} 4` : `M${x - 11} 4L${x + 12} -3`, 6),
    ]),
    closed: both((x) => [line(`M${x - 10} 0Q${x} 10 ${x + 10} 0`, 6)]),
    happy: both((x) => [line(`M${x - 10} 5Q${x} -11 ${x + 10} 5`, 6.2)]),
    side: both((x) => [E(x + 5, -1, 9.5, 12, { fill: 'ink' }), C(x + 2, -5.5, 4, { fill: 'white' }), hd(C(x + 8.6, 3.6, 1.7, { fill: 'white' }))]),
    wide: both((x) => [C(x, -1, 13, { fill: 'white', stroke: 'ink', sw: 5 }), C(x + 1, 0, 5.6, { fill: 'ink' }), hd(C(x - 0.8, -2, 1.8, { fill: 'white' }))]),
    teary: both((x) => [E(x, 0, 10, 12.5, { fill: 'ink' }), C(x - 3, -4.5, 5, { fill: 'white' }), C(x + 3.6, 4.6, 2.7, { fill: 'white' })]),
    stars: both((x) => [star(x, 0, 13, 'ink', 3.6)]),
  },
  brows: {
    none: [],
    up: both((x) => [line(`M${x - 9} -23Q${x} -31 ${x + 9} -23`)]),
    sus: [line('M-31 -26L-12 -19'), line('M12 -27Q21 -35 30 -27')],
    sad: [line('M-30 -19L-12 -26'), line('M12 -26L30 -19')],
    think: [line('M-30 -22L-12 -21'), line('M11 -28Q20 -36 30 -29')],
    worried: [line('M-31 -21Q-22 -23 -13 -29'), line('M13 -29Q22 -23 31 -21')],
  },
  mouth: {
    smile: [line('M-11 17Q0 29 11 17')],
    grin: [P('M-14 15L14 15Q14 33 0 33Q-14 33 -14 15Z', { fill: 'ink', stroke: 'ink', sw: 3.5 }), P('M-8 28.5Q0 22 8 28.5Q5 32 0 32Q-5 32 -8 28.5Z', { fill: '#FF7A9A' })],
    smirk: [line('M-10 22Q3 25 12 15')],
    o: [E(0, 22, 5, 6, { fill: 'ink' })],
    gasp: [E(0, 23, 7, 9, { fill: 'ink' })],
    frown: [line('M-10 26Q0 16 10 26')],
    wail: [P('M-14 31Q-13 15 0 15Q13 15 14 31Z', { fill: 'ink', stroke: 'ink', sw: 3.5 }), P('M-8 31Q0 25 8 31Z', { fill: '#FF7A9A' })],
    hmm: [line('M-3 22L11 19')],
    wobble: [line('M-12 23Q-8 18.5 -4 23T4 23T12 23', 5)],
  },
  // RIGHT socket only (the left one is mirrored). `joint` = redrawn over the body outline.
  arms: {
    rest: arm('M-8 -2Q2 4 5 12', [mitt(6, 15)]),
    wave: arm('M-8 0Q8 -2 12 -18', [mitt(13, -22)]),
    up: arm('M-8 0Q8 -8 11 -26', [mitt(12, -30)]),
    point: arm('M-8 0Q4 -2 10 -6', [...tube('M19 -10L32 -16', 15, 'body', 4.6), mitt(13, -8)]),
    aim: arm('M-8 0Q4 2 10 8', [...tube('M19 14L29 23', 15, 'body', 4.6), mitt(13, 10)]),
    droop: arm('M-8 0Q0 8 2 14', [mitt(2, 17)]),
    finger: arm('M-8 2Q12 6 12 -6', [...tube('M13 -14L14 -27', 15, 'body', 4.6), mitt(12, -9)]),
    hands: arm('M-8 0Q8 -2 9 -14', [mitt(9, -18, 13.5)]),
    none: { z: 'back', els: [], joint: [] },
  },
  // RIGHT hip only. `lift` moves the whole body (- = up, + = down); the legs decide the stance.
  legs: {
    stand: leg('M0 -6L2 13', [foot(5, 21)]),
    wide: leg('M0 -6L8 13', [foot(12, 21)]),
    shy: leg('M0 -6L-2 13', [foot(-1, 21, 13.5, 8.5, -18)]),
    tap: leg('M0 -6L3 11', [foot(9, 16, 13.5, 8, -24)]),
    tiptoe: leg('M0 -6L1 15', [foot(2, 24, 9.5, 10.5)], -6),
    hop: leg('M0 -4Q4 8 10 13', [foot(14, 18, 12.5, 8, 28)], -15),
    dangle: leg('M0 -4L-2 13', [foot(-2, 20, 11, 8, -14)], -11, 18),
    sit: { z: 'front', lift: 13, els: sole(2, 2, 28), joint: [] },
    none: { z: 'back', lift: 0, els: [], joint: [] },
  },
  props: {
    none: { at: 'prop', els: [] },
    zzz: {
      at: 'prop',
      els: [...tube('M-4 6H8L-4 20H8', 9.5, 'white', 3.2), ...tube('M16 -14H25L16 -3H25', 8.5, 'white', 3), ...tube('M32 -30H39L32 -22H39', 7.5, 'white', 2.8).map(hd)],
    },
    ask: {
      at: 'prop',
      els: [
        hd(C(-14, 26, 4.5, { fill: '#C8FF3D', stroke: 'ink', sw: 3.6 })),
        C(-4, 14, 6.5, { fill: '#C8FF3D', stroke: 'ink', sw: 4 }),
        C(16, -6, 19, { fill: '#C8FF3D', stroke: 'ink', sw: 5 }),
        P('M9.5 -10.5Q9.5 -18 16.5 -18Q23.5 -18 23.5 -11.5Q23.5 -6 16.5 -4V0', { stroke: 'ink', sw: 5 }),
        C(16.5, 6.5, 3, { fill: 'ink' }),
      ],
    },
    sparkles: {
      at: 'abs',
      els: [star(24, 48, 14, '#FFB547'), star(178, 40, 12, '#FF6DB5'), hd(star(16, 108, 10, '#5AC8FA')), hd(star(186, 110, 10, '#C8FF3D')), star(150, 14, 9, '#B48CFF')],
    },
    tear: { at: 'face', els: [drop(-29, 9, 16)] },
    tears: { at: 'face', els: [drop(-29, 9, 18), drop(29, 9, 18)] },
    sweat: { at: 'face', els: [drop(46, -34, 19)] },
  },
};

// ---------- MOODS: part names + a small whole-body pose (tilt°, dx, squash). Legs pick the stance. ----------
const MOODS = {
  happy: { eyes: 'open', mouth: 'smile', arms: ['rest', 'wave'], legs: ['stand', 'stand'] },
  accuse: { eyes: 'squint', brows: 'sus', mouth: 'smirk', arms: ['rest', 'point'], legs: ['wide', 'wide'], tilt: 4, dx: -10 },
  sleepy: { eyes: 'closed', mouth: 'o', arms: ['droop', 'droop'], legs: ['sit', 'sit'], prop: 'zzz', tilt: -4, squash: [1.04, 0.96] },
  celebrate: { eyes: 'happy', brows: 'up', mouth: 'grin', arms: ['up', 'up'], legs: ['hop', 'hop'], prop: 'sparkles' },
  sad: { eyes: 'teary', brows: 'sad', mouth: 'frown', arms: ['droop', 'droop'], legs: ['shy', 'shy'], prop: 'tear', squash: [1.04, 0.96] },
  think: { eyes: 'side', brows: 'think', mouth: 'hmm', arms: ['rest', 'finger'], legs: ['stand', 'tap'], prop: 'ask' },
  // the one everybody points at (jumps out of their skin)
  caught: { eyes: 'wide', brows: 'worried', mouth: 'wobble', arms: ['hands', 'hands'], legs: ['dangle', 'dangle'], prop: 'sweat' },
};

// Personality = a few part swaps on top of the shared moods.
const QUIRKS = {
  zeste: { accuse: { mouth: 'grin', brows: 'up' }, happy: { mouth: 'grin' } },
  bulle: {
    happy: { arms: ['rest', 'rest'], eyes: 'side', legs: ['shy', 'shy'] },
    accuse: { eyes: 'side', mouth: 'wobble', brows: 'worried', tilt: 2, legs: ['shy', 'shy'] },
  },
  pave: { happy: { mouth: 'smirk', arms: ['rest', 'finger'], eyes: 'lazy' }, think: { mouth: 'smirk', eyes: 'lazy', brows: 'up' } },
  paillette: {
    sad: { mouth: 'wail', prop: 'tears', arms: ['up', 'up'], eyes: 'closed', legs: ['sit', 'sit'], squash: [1, 1] },
    celebrate: { eyes: 'stars' },
    accuse: { mouth: 'gasp', brows: 'up', eyes: 'open', legs: ['tiptoe', 'tiptoe'] },
  },
  roupi: {
    happy: { eyes: 'closed', legs: ['sit', 'sit'] },
    accuse: { eyes: 'lazy', tilt: 2, legs: ['sit', 'sit'] },
    celebrate: { eyes: 'lazy', arms: ['rest', 'up'], legs: ['tiptoe', 'tiptoe'] },
    think: { eyes: 'closed', prop: 'zzz', legs: ['sit', 'sit'] },
  },
};

// ---------- output ----------
const round = (s) => {
  const o = {};
  for (const [k, v] of Object.entries(s)) o[k] = k === 'd' ? rp(v) : typeof v === 'number' ? r1(v) : v;
  return o;
};
const shapes = (list) => list.map(round);
const bodies = Object.fromEntries(
  Object.entries(BODIES).map(([who, b]) => [
    who,
    {
      ...b,
      d: rp(b.d),
      gloss: rp(b.gloss),
      back: shapes(b.back),
      front: shapes(b.front),
      faceUnder: shapes(b.faceUnder),
      faceExtras: shapes(b.faceExtras),
    },
  ]),
);
const limbs = (group) => Object.fromEntries(Object.entries(group).map(([k, p]) => [k, { ...p, els: shapes(p.els), joint: shapes(p.joint) }]));
const data = {
  v: 1,
  ink: INK,
  ground: GROUND,
  bodySw: 7.5,
  cast: Object.keys(BODIES),
  bodies,
  parts: {
    eyes: Object.fromEntries(Object.entries(PARTS.eyes).map(([k, v]) => [k, shapes(v)])),
    brows: Object.fromEntries(Object.entries(PARTS.brows).map(([k, v]) => [k, shapes(v)])),
    mouth: Object.fromEntries(Object.entries(PARTS.mouth).map(([k, v]) => [k, shapes(v)])),
    arms: limbs(PARTS.arms),
    legs: limbs(PARTS.legs),
    props: Object.fromEntries(Object.entries(PARTS.props).map(([k, p]) => [k, { ...p, els: shapes(p.els) }])),
  },
  moods: MOODS,
  quirks: QUIRKS,
};

// Every mood and quirk names existing parts.
const groupOf = { eyes: 'eyes', brows: 'brows', mouth: 'mouth', prop: 'props' };
for (const [where, m] of [...Object.entries(MOODS), ...Object.entries(QUIRKS).flatMap(([w, q]) => Object.entries(q).map(([k, v]) => [`${w}.${k}`, v]))]) {
  for (const [k, g] of Object.entries(groupOf)) if (m[k] && !PARTS[g][m[k]]) throw new Error(`${where}: unknown ${k} ${m[k]}`);
  for (const k of ['arms', 'legs']) for (const n of m[k] ?? []) if (!PARTS[k][n]) throw new Error(`${where}: unknown ${k} ${n}`);
}

const json = JSON.stringify(data) + '\n';
const root = new URL('../..', import.meta.url).pathname;
const targets = [`${root}app/src/lib/mascot-parts.json`, `${root}server/assets/mascot.json`];
if (process.argv.includes('--check')) {
  const stale = targets.filter((f) => {
    try {
      return readFileSync(f, 'utf8') !== json;
    } catch {
      return true;
    }
  });
  if (stale.length) {
    console.error(`mascot parts out of date (run node scripts/mascot-parts.mjs): ${stale.join(', ')}`);
    process.exit(1);
  }
  console.log('mascot parts up to date');
} else {
  for (const f of targets) writeFileSync(f, json);
  console.log(`mascot parts: ${json.length} bytes (${Object.keys(BODIES).length} bodies, ${Object.keys(MOODS).length} moods)`);
}
