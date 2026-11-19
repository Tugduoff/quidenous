/// <reference types="node" />
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { ACCENTS, type ArtPalette, ROOM_THEMES, themes } from '@/lib/tokens';
import { type ArtGrad, type ArtShape, scene, type Scene, SCENE_H } from '@/lib/utils/room-art';
import { ACCENT_IDS, CURATED_IDS, cleanTheme, curated, isCurated } from '@/lib/utils/room-theme';

const atLeast = (x: number, min: number, what: string) => assert.ok(x >= min, `${what}: ${x.toFixed(2)} < ${min}`);

const rgb = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
const lin = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const lum = (c: number[]) => 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
const ratio = (a: number[], b: number[]) => {
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};
const on = (text: string, surface: string) => ratio(rgb(text), rgb(surface));
const over = (base: number[], color: number[], alpha: number) => color.map((v, i) => base[i] * (1 - alpha) + v * alpha);

describe('accents', () => {
  for (const id of ACCENT_IDS) {
    const a = ACCENTS[id];
    test(`${id}: ink is AA text on the page and cards`, () => {
      atLeast(on(a.inkDark, themes.dark.bg), 4.5, 'ink on dark page');
      atLeast(on(a.inkDark, themes.dark.card2), 4.5, 'ink on dark card');
      atLeast(on(a.inkLight, themes.light.bg), 4.5, 'ink on light page');
      atLeast(on(a.inkLight, themes.light.card2), 4.5, 'ink on light card');
    });
    test(`${id}: text on an accent button is AA`, () => {
      atLeast(on(themes.dark.accTxt, a.acc), 4.5, 'text on accent');
    });
  }
});

describe('curated themes: colours', () => {
  for (const id of CURATED_IDS) {
    for (const scheme of ['dark', 'light'] as const) {
      const c = ROOM_THEMES[id][scheme];
      test(`${id} (${scheme}): text is AA on every surface`, () => {
        for (const surface of ['bg', 'card', 'card2'] as const) {
          atLeast(on(c.txt, c[surface]), 7, `text on ${surface}`);
          atLeast(on(c.mut, c[surface]), 4.5, `secondary text on ${surface}`);
          atLeast(on(c.accInk, c[surface]), 4.5, `accent ink on ${surface}`);
          atLeast(on(themes[scheme].tagInk, c[surface]), 4.5, `danger pink on ${surface}`);
        }
        atLeast(on(c.accInk, c.accSoft), 4.5, 'accent ink on its soft background');
        atLeast(on(c.txt, c.accSoft), 7, 'text on the soft accent background');
        atLeast(on(c.accTxt, c.acc), 4.5, 'text on an accent button');
        atLeast(on(c.tagTxt, c.tagBg), 4.5, 'text on the question chip');
      });
      test(`${id} (${scheme}): surfaces step away from the page`, () => {
        const step = (a: string, b: string) => Math.abs(lum(rgb(a)) - lum(rgb(b)));
        assert.ok(step(c.bg, c.card) > 0.004, 'card vs page');
        assert.ok(step(c.card, c.card2) > 0.004, 'card2 vs card');
        assert.ok(step(c.card2, c.line) > 0.004, 'line vs card2');
      });
    }
  }
});

function sample(g: ArtGrad, x: number, y: number): [number[], number] {
  let t: number;
  if (g.kind === 'linear') {
    const [dx, dy] = [g.x2 - g.x1, g.y2 - g.y1];
    t = ((x - g.x1) * dx + (y - g.y1) * dy) / (dx * dx + dy * dy);
  } else {
    t = Math.hypot(x - g.cx, y - g.cy) / g.r;
  }
  t = Math.max(0, Math.min(1, t));
  let i = 0;
  while (i < g.stops.length - 2 && t > g.stops[i + 1][0]) i++;
  const [a, b] = [g.stops[i], g.stops[i + 1]];
  const u = Math.max(0, Math.min(1, (t - a[0]) / (b[0] - a[0] || 1)));
  const [ca, cb] = [rgb(a[1]), rgb(b[1])];
  return [ca.map((v, k) => v + (cb[k] - v) * u), a[2] + (b[2] - a[2]) * u];
}

function polygon(d: string): [number, number][] | null {
  if (!/^M[-\d. ]+(L[-\d. ]+)+Z$/.test(d)) return null;
  const n = (d.match(/-?[\d.]+/g) ?? []).map(Number);
  return Array.from({ length: n.length / 2 }, (_, i) => [n[2 * i], n[2 * i + 1]]);
}

function covers(s: ArtShape, x: number, y: number): boolean {
  if (s.el === 'rect') return x >= s.x && x <= s.x + s.w && y >= s.y && y <= s.y + s.h;
  if (s.el === 'circle') return Math.hypot(x - s.cx, y - s.cy) <= s.r;
  const pts = polygon(s.d);
  if (!pts) return false;
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [[xi, yi], [xj, yj]] = [pts[i], pts[j]];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

function behind(sc: Scene, page: string, x: number, y: number): number[] {
  let c = rgb(page);
  for (const s of sc.shapes) {
    if (!s.grad || !covers(s, x, y)) continue;
    const grad = sc.grads.find((g) => g.id === s.grad);
    assert.ok(grad, `gradient ${s.grad} is defined`);
    const [color, alpha] = sample(grad, x, y);
    c = over(c, color, alpha * (s.op ?? 1));
  }
  return c;
}

describe('curated themes: scenes', () => {
  const WIDTHS = [320, 360, 390, 430, 768];
  for (const id of CURATED_IDS) {
    for (const scheme of ['dark', 'light'] as const) {
      const c = ROOM_THEMES[id][scheme];
      test(`${id} (${scheme}): the header's text stays AA over the scene`, () => {
        for (const w of WIDTHS) {
          const sc = scene(id, scheme === 'dark', w);
          assert.equal(sc.h, SCENE_H);
          for (let y = 22; y <= 46; y += 4)
            for (let x = 20; x <= 152; x += 6) {
              const at = behind(sc, c.bg, x, y);
              atLeast(ratio(rgb(c.txt), at), 4.5, `logo at ${x},${y} (width ${w})`);
              if (x >= 134) atLeast(ratio(rgb(c.accInk), at), 3, `the "?" at ${x},${y} (width ${w})`);
            }
          for (let y = 52; y <= 68; y += 4)
            for (let x = 20; x <= w - 160; x += 6) atLeast(ratio(rgb(c.mut), behind(sc, c.bg, x, y)), 4.5, `room name at ${x},${y} (width ${w})`);
        }
      });
      const art: ArtPalette = ROOM_THEMES[id].art[scheme];
      const far = art.far;
      if (far)
        test(`${id} (${scheme}): text reads on the far silhouette`, () => {
          atLeast(on(c.txt, far), 4.5, 'text on the far layer');
          atLeast(on(c.mut, far), 4.5, 'secondary text on the far layer');
        });
      test(`${id} (${scheme}): a scene stays small`, () => {
        for (const w of WIDTHS) assert.ok(scene(id, scheme === 'dark', w).shapes.length <= 32, `${scene(id, scheme === 'dark', w).shapes.length} shapes at ${w}`);
      });
    }
  }
  test('a scene is drawn once per theme, scheme and width', () => {
    assert.equal(scene('apero', true, 390), scene('apero', true, 390));
    assert.notEqual(scene('apero', true, 390), scene('apero', false, 390));
  });
});

describe('ids', () => {
  test('known ids pass, anything else is no theme', () => {
    assert.equal(cleanTheme('pyjama'), 'pyjama');
    assert.equal(cleanTheme('rose'), 'rose');
    assert.equal(cleanTheme('cosmos'), null);
    assert.equal(cleanTheme(''), null);
    assert.equal(cleanTheme(null), null);
    assert.equal(cleanTheme('__proto__'), null);
  });
  test('curated themes and plain accents are told apart', () => {
    assert.ok(isCurated('piscine'));
    assert.ok(!isCurated('violet'));
    assert.equal(curated('violet'), null);
    assert.equal(curated(null), null);
    assert.equal(curated('feu'), ROOM_THEMES.feu);
  });
  test('the server knows the same ids', () => {
    assert.deepEqual([...ACCENT_IDS, ...CURATED_IDS], ['lime', 'rose', 'bleu', 'orange', 'violet', 'menthe', 'apero', 'feu', 'karaoke', 'pyjama', 'arcade', 'piscine']);
  });
});
