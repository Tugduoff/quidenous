// Mascot rig tests. Run from app/: `bun test src/lib/mascot.test.ts` (CI checks the generated JSON with node alone).
/// <reference types="node" />
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { CAST, compose, darken, type Group, MASCOT, MOODS, type Pose, resolvePose } from '@/lib/mascot';

const TOKENS = new Set(['ink', 'body', 'shade', 'cheek', 'white', 'acc2']);
const groups = (c: ReturnType<typeof compose>): Group[] => [...c.back, ...c.over, c.under, c.eyes, ...(c.blink ? [c.blink] : []), c.face, ...c.front, ...(c.prop ? [c.prop] : [])];

describe('mascot parts', () => {
  test('the generated JSON (app and server copies) is up to date with scripts/mascot-parts.mjs', () => {
    const r = spawnSync('node', ['scripts/mascot-parts.mjs', '--check'], { cwd: `${__dirname}/../..`, encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr || r.stdout);
  });

  test('stays small', () => {
    assert.ok(JSON.stringify(MASCOT).length < 20_000);
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
    // brows `up` (2 lines) are gone from the face
    assert.equal(hi.face.shapes.length - lo.face.shapes.length, 2 + 1 /* glasses temples */);
    assert.ok(lo.over.length < hi.over.length, 'joints dropped');
  });

  test('moods take the shared parts, then the character quirks', () => {
    assert.deepEqual(resolvePose('zeste', 'accuse').mouth, 'grin');
    assert.equal(resolvePose('bulle', 'accuse').mouth, 'wobble');
    assert.equal(resolvePose('paillette', 'celebrate').eyes, 'stars');
    // Roupi never stands up: sitting lowers the body.
    assert.deepEqual(resolvePose('roupi', 'accuse').legs, ['sit', 'sit']);
    assert.equal(resolvePose('roupi', 'accuse').lift, 13);
    // A hop lifts it, and the shadow shrinks on the ground.
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
