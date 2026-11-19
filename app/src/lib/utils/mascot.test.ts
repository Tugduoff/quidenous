/// <reference types="node" />
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { CAST, compose, darken, type Group, MASCOT, MOODS, type Pose, resolvePose } from '@/lib/utils/mascot';

const TOKENS = new Set(['ink', 'body', 'shade', 'cheek', 'white', 'acc2', 'wool', 'wool2']);
const groups = (c: ReturnType<typeof compose>): Group[] =>
  [...c.back, ...c.over, c.under, c.eyes, c.blink, c.face, ...c.front, c.prop, c.scarf, c.hat, c.hatTop].filter((g): g is Group => !!g);

describe('mascot parts', () => {
  test('the generated JSON (app and server copies) is up to date with scripts/mascot-parts.mjs', () => {
    const r = spawnSync('node', ['scripts/mascot-parts.mjs', '--check'], { cwd: `${__dirname}/../../..`, encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr || r.stdout);
  });

  test('stays small, the winter outfit included', () => {
    assert.ok(JSON.stringify(MASCOT).length < 26_000);
  });
});

describe('compose', () => {
  test('every character draws every mood, with resolved colours and a bounded number of elements', () => {
    for (const who of CAST)
      for (const mood of MOODS) {
        const hi = compose(who, mood);
        const lo = compose(who, mood, { lod: 'lo' });
        for (const g of groups(hi))
          for (const s of g.shapes) {
            assert.ok(!s.fill || !TOKENS.has(s.fill), `${who}/${mood}: unresolved fill ${s.fill}`);
            assert.ok(!s.stroke || !TOKENS.has(s.stroke), `${who}/${mood}: unresolved stroke ${s.stroke}`);
          }
        assert.ok(hi.nodes <= 52, `${who}/${mood}: ${hi.nodes} elements`);
        assert.ok(lo.nodes < hi.nodes, `${who}/${mood}: lo ${lo.nodes} >= hi ${hi.nodes}`);
        assert.ok(lo.nodes <= 38, `${who}/${mood}: lo ${lo.nodes} elements`);
      }
  });

  test('the small level of detail drops brows, gloss, shading, joints and every detail shape', () => {
    const hi = compose('pave', 'think');
    const lo = compose('pave', 'think', { lod: 'lo' });
    assert.ok(hi.fill.gloss);
    assert.equal(lo.fill.gloss, null);
    for (const g of groups(lo)) for (const s of g.shapes) assert.ok(!s.hd, 'a detail shape survived');
    const upBrowLines = 2;
    const glassesTemples = 1;
    assert.equal(hi.face.shapes.length - lo.face.shapes.length, upBrowLines + glassesTemples);
    assert.ok(lo.over.length < hi.over.length, 'joints dropped');
  });

  test('moods take the shared parts, then the character quirks', () => {
    assert.deepEqual(resolvePose('zeste', 'accuse').mouth, 'grin');
    assert.equal(resolvePose('bulle', 'accuse').mouth, 'wobble');
    assert.equal(resolvePose('paillette', 'celebrate').eyes, 'stars');
    assert.deepEqual(resolvePose('roupi', 'accuse').legs, ['sit', 'sit']);
    assert.equal(resolvePose('roupi', 'accuse').lift, 13);
    const hop = compose('zeste', 'celebrate');
    assert.equal(hop.lift, -15);
    assert.ok(hop.shadow.air && hop.shadow.rx < compose('zeste', 'happy').shadow.rx);
  });

  test('only open eyes blink', () => {
    assert.ok(compose('zeste', 'happy').blink);
    assert.equal(compose('zeste', 'celebrate').blink, null);
    assert.equal(compose('roupi', 'sleepy').blink, null);
  });

  test('left limbs are the right ones mirrored; accusing points the way the finger does', () => {
    const c = compose('zeste', 'accuse');
    const limbs = c.back.filter((g) => g.tr?.startsWith('translate('));
    assert.ok(limbs.some((g) => g.tr?.endsWith('scale(-1 1)')));
    assert.equal(c.point, 1);
    const pose: Pose = { eyes: 'lazy', mouth: 'smirk', arms: ['point', 'rest'], legs: ['sit', 'sit'] };
    assert.equal(compose('roupi', pose).point, -1);
    assert.equal(compose('bulle', 'sad').point, 0);
  });

  test('props: the mood default, an override, or none', () => {
    assert.equal(compose('roupi', 'sleepy').prop?.kind, 'zzz');
    assert.equal(compose('pave', 'think').prop?.kind, 'ask');
    assert.equal(compose('pave', 'think', { prop: 'none' }).prop, null);
    assert.equal(compose('paillette', 'celebrate').prop?.at, 'abs');
  });

  test('tint repaints the body and derives its shade', () => {
    const c = compose('bulle', 'happy', { tint: '#FF6DB5' });
    assert.equal(c.fill.color, '#FF6DB5');
    assert.equal(c.fill.shade, darken('#FF6DB5'));
    assert.equal(darken('#FFFFFF', 0.5), '#808080');
  });

  test('results are cached: by arguments for moods, by identity for custom poses', () => {
    assert.equal(compose('zeste', 'happy'), compose('zeste', 'happy'));
    assert.notEqual(compose('zeste', 'happy'), compose('zeste', 'happy', { lod: 'lo' }));
    const pose: Pose = { eyes: 'open', mouth: 'smile', arms: ['rest', 'rest'], legs: ['stand', 'stand'] };
    assert.equal(compose('bulle', pose), compose('bulle', pose));
  });
});

describe('seasonal outfit', () => {
  test('winter adds a beanie and a scarf to every character (Roupi keeps his nightcap), resolved and bounded', () => {
    for (const who of CAST)
      for (const mood of MOODS) {
        const plain = compose(who, mood);
        const hi = compose(who, mood, { outfit: 'winter' });
        const lo = compose(who, mood, { lod: 'lo', outfit: 'winter' });
        assert.equal(plain.scarf, null);
        assert.equal(plain.hat, null);
        assert.equal(plain.hatTop, null);
        const bandTailAndKnot = 3;
        assert.ok(hi.scarf && hi.scarf.shapes.length >= bandTailAndKnot, `${who}/${mood}: no scarf`);
        const domeCuffAndKnit = 5;
        if (who === 'roupi') assert.equal(hi.hat, null);
        else assert.ok(hi.hat && hi.hat.shapes.length >= domeCuffAndKnit, `${who}/${mood}: no beanie`);
        for (const g of groups(hi))
          for (const s of g.shapes) {
            assert.ok(!s.fill || !TOKENS.has(s.fill), `${who}/${mood}: unresolved fill ${s.fill}`);
            assert.ok(!s.stroke || !TOKENS.has(s.stroke), `${who}/${mood}: unresolved stroke ${s.stroke}`);
          }
        const outfitBudget = 14;
        assert.ok(hi.nodes > plain.nodes && hi.nodes <= plain.nodes + outfitBudget, `${who}/${mood}: ${hi.nodes} elements with the outfit, ${plain.nodes} without`);
        assert.equal(lo.scarf, null, 'small: no scarf');
        for (const g of [lo.hat, lo.hatTop]) for (const s of g?.shapes ?? []) assert.ok(!s.hd, 'a knit detail survived');
        assert.ok(lo.nodes <= 41, `${who}/${mood}: lo ${lo.nodes} elements with the outfit`);
      }
  });

  test('the outfit stays inside the drawing box, even at the top of a hop', () => {
    const ys = (d: string) => (d.match(/-?\d*\.?\d+/g) ?? []).map(Number).filter((_, i) => i % 2 === 1);
    for (const who of CAST) {
      const c = compose(who, 'celebrate', { outfit: 'winter' });
      for (const g of [c.hat, c.hatTop, c.scarf])
        for (const s of g?.shapes ?? []) {
          const half = (s.sw ?? 0) / 2;
          if (s.el === 'circle') assert.ok((s.cy ?? 0) - (s.r ?? 0) - half + c.lift >= 0, `${who}: a pom-pom leaves the box`);
          const d = s.el === 'path' ? (s.d ?? '') : '';
          const arcsCarryRadiiNotPoints = d.includes('A');
          if (!arcsCarryRadiiNotPoints) for (const y of ys(d)) assert.ok(y - half + c.lift >= 0 && y + half + c.lift <= 200, `${who}: ${y} leaves the box`);
        }
    }
  });

  test("the beanie hides tufts and stems (Zeste's pokes through it); the outfit is cached apart and follows the body", () => {
    assert.ok(compose('zeste', 'happy').back.length > compose('zeste', 'happy', { outfit: 'winter' }).back.length);
    assert.ok(compose('zeste', 'happy', { outfit: 'winter' }).hatTop, 'the stem sticks out of the beanie');
    assert.ok(compose('roupi', 'happy', { outfit: 'winter' }).hatTop, 'snow on the nightcap');
    assert.equal(compose('bulle', 'happy', { outfit: 'winter' }).hatTop, null);
    assert.notEqual(compose('zeste', 'happy'), compose('zeste', 'happy', { outfit: 'winter' }));
    assert.equal(compose('zeste', 'happy', { outfit: 'winter' }), compose('zeste', 'happy', { outfit: 'winter' }));
    const pose: Pose = { eyes: 'open', mouth: 'smile', arms: ['rest', 'rest'], legs: ['stand', 'stand'] };
    assert.notEqual(compose('bulle', pose), compose('bulle', pose, { outfit: 'winter' }));
    const wool = new Set(CAST.map((who) => compose(who, 'happy', { outfit: 'winter' }).scarf?.shapes[0].fill));
    assert.equal(wool.size, CAST.length, 'each friend knits in its own colour');
  });
});
