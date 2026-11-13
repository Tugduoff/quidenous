// Builds the avatar parts (the art of the avatar builder) into the one JSON shared by the app and the server:
// app/src/lib/avatar-parts.json (TS renderer) and server/assets/avatar.json (Rust renderer, share cards).
// Run from app/: `node scripts/avatar-parts.mjs`. Shapes live in a 100×100 box clipped to a circle.
import { writeFileSync } from 'node:fs';

const INK = '#1B1B22';
const WHITE = '#FFFFFF';
const PINK = '#FF4FA3';
const TONGUE = '#FF6B8A';
const GOLD = '#FFC83D';

const path = (d, fill, extra = {}) => ({ el: 'path', d, ...(fill ? { fill } : {}), ...extra });
const line = (d, stroke = INK, sw = 2.6, extra = {}) => ({ el: 'path', d, stroke, sw, ...extra });
const circle = (cx, cy, r, fill, extra = {}) => ({ el: 'circle', cx, cy, r, ...(fill ? { fill } : {}), ...extra });
const ellipse = (cx, cy, rx, ry, fill, extra = {}) => ({ el: 'ellipse', cx, cy, rx, ry, fill, ...extra });
const rect = (x, y, w, h, rx, fill, extra = {}) => ({ el: 'rect', x, y, w, h, ...(rx ? { rx } : {}), ...(fill ? { fill } : {}), ...extra });
const back = (s) => ({ ...s, layer: 'back' });

// Both eyes from one drawing of the left one (centred on its x), the right one mirrored or shifted.
const EL = 41;
const ER = 59;
const EY = 49;
/** Translate a path's absolute coordinates by dx (x only): every even number in the list. */
const shiftX = (d, dx) => {
  let i = 0;
  return d.replace(/-?\d+(\.\d+)?/g, (n) => (i++ % 2 === 0 ? String(+(+n + dx).toFixed(2)) : n));
};
/** Mirror a path around x = 50. */
const mirror = (d) => {
  let i = 0;
  return d.replace(/-?\d+(\.\d+)?/g, (n) => (i++ % 2 === 0 ? String(+(100 - +n).toFixed(2)) : n));
};
const pair = (make) => [...make(EL, 1), ...make(ER, -1)];

const skin = [
  ['#FFE1C8', '#F0BF9C'],
  ['#F5C79E', '#E2A47A'],
  ['#E2A574', '#C9875A'],
  ['#C1824F', '#A2673A'],
  ['#8E5A37', '#744628'],
  ['#5E3925', '#472A1A'],
  ['#B4E08A', '#92C26A'],
];

const hairColor = [
  ['#2A2630', '#18161C'],
  ['#55392A', '#3E291D'],
  ['#8F5C3B', '#70462B'],
  ['#F2C960', '#D6AA3F'],
  ['#DD6A2F', '#B95222'],
  ['#CDD0D9', '#A4A8B6'],
  ['#FF7EB8', '#E2589A'],
  ['#5B8CFF', '#3D6BE0'],
  ['#B5E835', '#93C51F'],
  ['#9B6BFF', '#7A4AE0'],
];

const mix = (hex, to, k) => {
  const a = hex.match(/\w\w/g).map((h) => parseInt(h, 16));
  const b = to.match(/\w\w/g).map((h) => parseInt(h, 16));
  return (
    '#' +
    a
      .map((v, i) =>
        Math.round(v + (b[i] - v) * k)
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
      .toUpperCase()
  );
};
const BGS = ['#FF7A45', '#5BC8FF', '#FF4FA3', '#FFB547', '#B48CFF', '#5CF2B0', '#FF6B6B', '#7AA2FF', '#C8FF3D', '#2EC4B6', '#FFE14D', '#2E2E38'];
const bg = BGS.map((c) => [c, c === '#2E2E38' ? '#5B5B6E' : mix(c, '#121216', 0.42)]);

const base = [
  path('M42 62 L42 80 Q50 86 58 80 L58 62 Z', 'skinShade'),
  path('M12 106 C12 88 28 80 41 78.5 Q50 85 59 78.5 C72 80 88 88 88 106 Z', 'shirt'),
];

const ears = (x, y = 52) => [circle(x, y, 5.5, 'skin'), circle(100 - x, y, 5.5, 'skin')];
const nose = line('M48.4 54.2 Q50 56.4 51.6 54.2', 'skinShade', 2.2);
const face = [
  [...ears(27.5), ellipse(50, 50, 23, 25, 'skin'), nose],
  [...ears(29.5), ellipse(50, 50, 21, 27, 'skin'), nose],
  [...ears(27, 53), rect(27, 25, 46, 50, 17, 'skin'), nose],
  [...ears(28, 50), path('M27 46 C27 31 37 24 50 24 C63 24 73 31 73 46 C73 62 62 76 50 76 C38 76 27 62 27 46 Z', 'skin'), nose],
  [...ears(25.5), ellipse(50, 51, 25, 24, 'skin'), nose],
];

const heart = (cx, cy, fill) =>
  path(
    `M${cx} ${cy + 3.6} C${cx - 5.4} ${cy - 0.2} ${cx - 4.4} ${cy - 5} ${cx} ${cy - 2} C${cx + 4.4} ${cy - 5} ${cx + 5.4} ${cy - 0.2} ${cx} ${cy + 3.6} Z`,
    fill,
  );
const eyes = [
  pair((x) => [circle(x, EY, 3.3, INK)]),
  pair((x) => [line(`M${x - 3.6} ${EY + 1} Q${x} ${EY - 3.6} ${x + 3.6} ${EY + 1}`)]),
  pair((x) => [circle(x, EY, 5, WHITE), circle(x + 0.4, EY + 0.4, 3.2, INK), circle(x + 1.6, EY - 0.9, 1.1, WHITE)]),
  [circle(EL, EY, 3.3, INK), line(`M${ER - 3.6} ${EY + 0.5} Q${ER} ${EY - 3} ${ER + 3.6} ${EY + 0.5}`)],
  pair((x) => [path(`M${x - 4} ${EY} A4 4 0 0 0 ${x + 4} ${EY} Z`, INK), line(`M${x - 4.8} ${EY} H${x + 4.8}`, INK, 2.2)]),
  pair((x) => [circle(x, EY, 4.6, WHITE, { stroke: INK, sw: 1.6 }), circle(x, EY, 1.8, INK)]),
  pair((x) => [heart(x, EY, PINK)]),
  pair((x) => [ellipse(x, EY, 4.6, 4, WHITE), circle(x + 1.8, EY + 0.3, 2.4, INK)]),
  pair((x, s) => [circle(x, EY, 3.3, INK), line(`M${x - 2.6 * s} ${EY - 2.4} L${x - 4.6 * s} ${EY - 4.6}`, INK, 1.8)]),
];

const BY = 41.5;
const brow = (dl, dr = mirror(dl)) => [line(dl, 'hairShade', 2.6), line(dr, 'hairShade', 2.6)];
const brows = [
  [],
  brow(`M37 ${BY} Q41 ${BY - 2} 45 ${BY}`),
  brow(`M37 ${BY - 1} Q41 ${BY - 5} 45 ${BY - 2}`),
  brow(`M37 ${BY - 2.5} L45 ${BY + 0.8}`),
  brow(`M37 ${BY + 0.5} L45 ${BY - 2.2}`),
  [line(`M36.5 ${BY} Q41 ${BY - 2.6} 45.5 ${BY}`, 'hairShade', 3.8), line(mirror(`M36.5 ${BY} Q41 ${BY - 2.6} 45.5 ${BY}`), 'hairShade', 3.8)],
  [line(`M37 ${BY} Q41 ${BY - 2} 45 ${BY}`, 'hairShade', 2.6), line(`M55 ${BY - 2} Q59 ${BY - 6} 63 ${BY - 3}`, 'hairShade', 2.6)],
];

const MY = 61;
const mouth = [
  [line(`M43 ${MY - 1} Q50 ${MY + 6} 57 ${MY - 1}`)],
  [
    path(`M41 ${MY - 2} H59 Q58.5 ${MY + 9} 50 ${MY + 9} Q41.5 ${MY + 9} 41 ${MY - 2} Z`, INK),
    rect(43.5, MY - 2, 13, 3, 1, WHITE),
    ellipse(50, MY + 6, 4.4, 2.2, TONGUE),
  ],
  [line(`M44 ${MY + 1} H56`)],
  [ellipse(50, MY + 2, 3.6, 4.6, INK)],
  [line(`M44 ${MY + 1} Q51 ${MY + 3} 56.5 ${MY - 1.5}`)],
  [path(`M46.5 ${MY + 2} Q46.5 ${MY + 9} 50.5 ${MY + 9} Q54.5 ${MY + 9} 54.5 ${MY + 2} Z`, TONGUE), line(`M43 ${MY - 1} Q50 ${MY + 5} 57 ${MY - 1}`)],
  [
    path(`M39.5 ${MY - 3} H60.5 Q60.5 ${MY + 11} 50 ${MY + 11} Q39.5 ${MY + 11} 39.5 ${MY - 3} Z`, INK),
    rect(42, MY - 3, 16, 3.2, 1, WHITE),
    ellipse(50, MY + 7.6, 5.6, 2.8, TONGUE),
  ],
  [line(`M43 ${MY + 3} Q50 ${MY - 3} 57 ${MY + 3}`)],
  [line(`M43 ${MY} Q46.5 ${MY + 3.5} 50 ${MY} Q53.5 ${MY + 3.5} 57 ${MY}`)],
  [rect(41, MY - 3, 18, 8, 3, WHITE, { stroke: INK, sw: 2 }), line(`M41 ${MY + 1} H59`, INK, 1.4)],
];

const glasses = [
  [],
  [
    circle(EL, EY, 6.6, undefined, { stroke: INK, sw: 2.2 }),
    circle(ER, EY, 6.6, undefined, { stroke: INK, sw: 2.2 }),
    line('M47.6 48.6 Q50 46.8 52.4 48.6', INK, 2.2),
    line('M34.4 48 L28 46.5', INK, 2.2),
    line('M65.6 48 L72 46.5', INK, 2.2),
  ],
  [
    rect(33.5, 43.5, 15, 11.5, 3, undefined, { stroke: INK, sw: 2.2 }),
    rect(51.5, 43.5, 15, 11.5, 3, undefined, { stroke: INK, sw: 2.2 }),
    line('M48.5 48 H51.5', INK, 2.2),
    line('M33.5 47 L28 46', INK, 2.2),
    line('M66.5 47 L72 46', INK, 2.2),
  ],
  [
    path('M32.5 44 H48.5 V49 Q48.5 55.5 41 55.5 Q32.5 55.5 32.5 49 Z', INK),
    path(mirror('M32.5 44 H48.5 V49 Q48.5 55.5 41 55.5 Q32.5 55.5 32.5 49 Z'), INK),
    line('M31 44.2 H69', INK, 2.4),
    line('M35.5 47 L39 47', WHITE, 1.6, { op: 0.55 }),
    line('M53.5 47 L57 47', WHITE, 1.6, { op: 0.55 }),
  ],
  [
    circle(EL, EY, 7, WHITE, { stroke: PINK, sw: 2.8, op: 1 }),
    circle(ER, EY, 7, WHITE, { stroke: PINK, sw: 2.8 }),
    line('M47.6 48.4 Q50 46.4 52.4 48.4', PINK, 2.6),
  ],
];
// The pink frames: lenses barely tinted, eyes stay visible.
glasses[4][0] = circle(EL, EY, 7, undefined, { stroke: PINK, sw: 2.8 });
glasses[4][1] = circle(ER, EY, 7, undefined, { stroke: PINK, sw: 2.8 });

const MOUSTACHE = 'M40.5 58.5 Q45 53.6 50 56.4 Q55 53.6 59.5 58.5 Q55 60.2 50 58.4 Q45 60.2 40.5 58.5 Z';
const BEARD =
  'M26.5 49 C26.5 67 36 80 50 80 C64 80 73.5 67 73.5 49 L69.5 52 C68.5 60 64 64.5 58.5 64.5 Q50 61.5 41.5 64.5 C36 64.5 31.5 60 30.5 52 Z';
const beard = [
  [],
  [path(MOUSTACHE, 'hair')],
  [path(BEARD, 'hair'), path(MOUSTACHE, 'hair')],
  [path('M44.5 66.5 Q50 64.8 55.5 66.5 Q55.5 74 50 75 Q44.5 74 44.5 66.5 Z', 'hair'), path(MOUSTACHE, 'hair')],
  [path(BEARD, 'hairShade', { op: 0.32 }), path(MOUSTACHE, 'hairShade', { op: 0.32 })],
];

const TOP = 'M26 46 C25 28 37 19 50 19 C63 19 75 28 74 46 C71 38 66 32 50 31 C34 32 29 38 26 46 Z';
const SHAVED = 'M27 44 C26 29 37 21 50 21 C63 21 74 29 73 44 C70 36 62 32 50 32 C38 32 30 36 27 44 Z';
const hair = [
  [ellipse(41, 31, 5, 2.4, WHITE, { op: 0.28 })],
  [path('M25.5 47 C23.5 27 36 17 51 17 C66 17 77.5 27 74.5 47 C72.5 40 70 35 66 33 C58 36.5 44 36.5 34 33 C30 35 27.5 40 25.5 47 Z', 'hair')],
  [path('M25.5 48 C22 26 39 14 56 17.5 C70 20 79 32 74.5 48 C72.5 41 70.5 37 67.5 34.5 C60 40 44 42 34 37.5 C30 40 27.5 43.5 25.5 48 Z', 'hair')],
  [
    path(
      'M26 46 L24.5 30 L31 31.5 L31.5 20 L39 25.5 L43.5 13.5 L50 23 L56.5 13.5 L61 25.5 L68.5 20 L69 31.5 L75.5 30 L74 46 C72 39 69 35 64 33 C56 36 44 36 36 33 C31 35 28 39 26 46 Z',
      'hair',
    ),
  ],
  [
    back(circle(50, 31, 22, 'hair')),
    back(circle(30.5, 40, 12, 'hair')),
    back(circle(69.5, 40, 12, 'hair')),
    back(circle(27, 54, 9, 'hair')),
    back(circle(73, 54, 9, 'hair')),
    circle(35.5, 30.5, 7.5, 'hair'),
    circle(45, 26.5, 8, 'hair'),
    circle(55, 26.5, 8, 'hair'),
    circle(64.5, 30.5, 7.5, 'hair'),
  ],
  [
    back(path('M23.5 48 C21.5 26 36 16.5 50 16.5 C64 16.5 78.5 26 76.5 48 L79 88 Q64 92 50 90 Q36 92 21 88 Z', 'hair')),
    path(
      'M50 18 C34 18 24.5 28 24.5 46 L24.5 73 L31 73 C30 58 32 46 37 38.5 C42 33.5 47 31.5 50 31.5 C53 31.5 58 33.5 63 38.5 C68 46 70 58 69 73 L75.5 73 L75.5 46 C75.5 28 66 18 50 18 Z',
      'hair',
    ),
  ],
  [
    back(path('M23 46 C21 25 36 16.5 50 16.5 C64 16.5 79 25 77 46 L78 69 Q50 75 22 69 Z', 'hair')),
    path('M24.5 46 C23.5 26 37 17 50 17 C63 17 76.5 26 75.5 46 C73 41 70.5 39.5 66 39.5 L34 39.5 C29.5 39.5 27 41 24.5 46 Z', 'hair'),
  ],
  [back(circle(50, 15, 9.5, 'hair')), path(TOP, 'hair'), line('M43 22.5 Q50 20 57 22.5', 'hairShade', 2.2)],
  [back(path('M66 25 C84 26 88 48 81 70 C78 58 74 46 66 39 Z', 'hair')), path(TOP, 'hair'), back(circle(70, 27.5, 3.5, 'hairShade'))],
  [path(SHAVED, 'hair', { op: 0.3 }), path('M43 33 C42 22 44.5 11.5 50 7.5 C55.5 11.5 58 22 57 33 Z', 'hair')],
  [path(SHAVED, 'hair', { op: 0.6 })],
  [back(circle(28.5, 25, 9.5, 'hair')), back(circle(71.5, 25, 9.5, 'hair')), path(TOP, 'hair'), line('M50 19 V27', 'hairShade', 2)],
  [
    path(
      'M25 50 C21 39 25 24 38 18.5 C46 14 58 14.5 66 19.5 C76.5 25.5 80 39 75 50 C74 44 72 40 69 38 C68 41.5 64 41.5 62 37.5 C59 40.5 54 40.5 52 36.5 C49 40.5 44 40.5 42 36.5 C39 40.5 34 40.5 33 37.5 C30 39.5 27 43 25 50 Z',
      'hair',
    ),
  ],
];

const extra = [
  [],
  [path('M25 40 C25 21.5 37 13 50 13 C63 13 75 21.5 75 40 Z', PINK), ellipse(50, 40.5, 30, 5, '#D63487'), circle(50, 13.5, 2.2, '#D63487')],
  [path('M25 38 C25 19 37 10.5 50 10.5 C63 10.5 75 19 75 38 Z', '#FF7A45'), rect(22.5, 32, 55, 10, 5, '#E0592A'), circle(50, 9.5, 5.5, '#FFB547')],
  [circle(27.5, 61, 3, undefined, { stroke: GOLD, sw: 1.9 }), circle(72.5, 61, 3, undefined, { stroke: GOLD, sw: 1.9 })],
  [
    line('M24 50 C21.5 22 36 11.5 50 11.5 C64 11.5 78.5 22 76 50', INK, 4.4),
    rect(18.5, 43, 10, 16, 4.5, '#5BC8FF'),
    rect(71.5, 43, 10, 16, 4.5, '#5BC8FF'),
  ],
  [ellipse(35, 57, 4.6, 2.6, TONGUE, { op: 0.5 }), ellipse(65, 57, 4.6, 2.6, TONGUE, { op: 0.5 })],
  [
    ...[
      [37, 56],
      [40.5, 58.2],
      [43.5, 55.4],
      [56.5, 55.4],
      [59.5, 58.2],
      [63, 56],
    ].map(([x, y]) => circle(x, y, 1, '#8A4B2A', { op: 0.6 })),
  ],
  [path('M33.5 24 L35 8 L42.5 16 L50 4.5 L57.5 16 L65 8 L66.5 24 Z', GOLD), circle(50, 18.5, 2.2, '#FF4FA3')],
  [
    ...[0, 72, 144, 216, 288].map((a) =>
      circle(
        +(70 + 4.2 * Math.cos(((a - 90) * Math.PI) / 180)).toFixed(2),
        +(27 + 4.2 * Math.sin(((a - 90) * Math.PI) / 180)).toFixed(2),
        3.6,
        '#FF7EB8',
      ),
    ),
    circle(70, 27, 2.6, GOLD),
  ],
  [line('M25 37 Q50 26 75 37', '#C8FF3D', 5)],
];

const fields = ['face', 'skin', 'hair', 'hairColor', 'eyes', 'brows', 'mouth', 'glasses', 'beard', 'extra', 'bg'];
//            face skin hair hc eyes brows mouth gl beard extra bg
const presets = [
  [0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 0],
  [3, 0, 5, 3, 2, 1, 0, 0, 0, 5, 2],
  [2, 3, 3, 0, 1, 2, 1, 0, 0, 0, 1],
  [1, 4, 4, 0, 0, 1, 0, 1, 0, 3, 5],
  [0, 2, 6, 4, 0, 1, 2, 2, 0, 6, 9],
  [2, 1, 0, 0, 7, 3, 4, 3, 2, 0, 11],
  [0, 5, 11, 0, 2, 1, 6, 0, 0, 3, 3],
  [4, 0, 7, 6, 6, 2, 0, 0, 0, 5, 4],
  [3, 2, 2, 1, 3, 6, 4, 0, 1, 0, 7],
  [0, 1, 9, 8, 5, 3, 9, 0, 0, 0, 6],
  [1, 0, 12, 5, 0, 1, 7, 1, 2, 0, 10],
  [0, 3, 8, 2, 8, 1, 0, 0, 0, 3, 2],
  [4, 4, 10, 0, 1, 1, 1, 0, 3, 4, 8],
  [2, 6, 0, 0, 5, 2, 3, 0, 0, 7, 3],
  [3, 1, 5, 7, 4, 4, 8, 4, 0, 0, 0],
  [0, 0, 1, 4, 0, 5, 5, 0, 0, 1, 7],
  [1, 3, 6, 9, 2, 0, 0, 0, 0, 8, 5],
  [4, 2, 3, 3, 1, 2, 6, 3, 0, 2, 1],
  [0, 5, 4, 1, 0, 1, 0, 2, 4, 9, 4],
  [2, 0, 2, 2, 3, 6, 4, 0, 4, 0, 8],
].map((p) => p.join('.'));

const data = { v: 1, fields, skin, hairColor, bg, base, face, hair, eyes, brows, mouth, glasses, beard, extra, presets };
for (const p of presets) {
  const parts = p.split('.').map(Number);
  parts.forEach((n, i) => {
    if (!(n < data[fields[i]].length)) throw new Error(`preset ${p}: ${fields[i]} ${n} out of range`);
  });
}
const json = JSON.stringify(data) + '\n';
const root = new URL('../..', import.meta.url).pathname;
writeFileSync(`${root}app/src/lib/avatar-parts.json`, json);
writeFileSync(`${root}server/assets/avatar.json`, json);
console.log(`avatar parts: ${json.length} bytes, ${fields.map((f) => `${f} ${data[f].length}`).join(', ')}`);
