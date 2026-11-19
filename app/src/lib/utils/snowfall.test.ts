import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { CRYSTAL_DEPTH, depthCounts, flakeAt, SNOW_LOOP_S, SNOW_PERIODS_S, snowLayout } from '@/lib/utils/snowfall';
import { snowdrift } from '@/lib/utils/snowdrift';

const BAND = 150;

describe('snowfall', () => {
  test('a light fall: 14 flakes at three depths, two or three of them near crystals', () => {
    assert.deepEqual(depthCounts(14), [6, 5, 3]);
    const flakes = snowLayout(14, 7);
    assert.equal(flakes.length, 14);
    assert.deepEqual(new Set(flakes.map((f) => f.depth)), new Set([0, 1, 2]));
    assert.equal(flakes.filter((f) => f.depth === CRYSTAL_DEPTH).length, 3);
    for (const f of flakes) assert.ok(f.left >= 0 && f.left < 1, `column ${f.left}`);
  });

  test('the same seed lays the same flakes out, another seed does not', () => {
    assert.deepEqual(snowLayout(14, 7), snowLayout(14, 7));
    assert.notDeepEqual(snowLayout(14, 7), snowLayout(14, 11));
  });

  test('near flakes are bigger and fall faster than far ones', () => {
    const flakes = snowLayout(20, 11);
    const far = flakes.filter((f) => f.depth === 0);
    const near = flakes.filter((f) => f.depth === CRYSTAL_DEPTH);
    assert.ok(Math.max(...far.map((f) => f.r)) < Math.min(...near.map((f) => f.r)));
    assert.ok(Math.min(...far.map((f) => f.fall)) > Math.max(...near.map((f) => f.fall)));
  });

  test('every period divides the clock loop, so the fall has no seam', () => {
    for (const period of SNOW_PERIODS_S) assert.equal(SNOW_LOOP_S % period, 0, `${period} s`);
    for (const f of snowLayout(20, 11)) {
      const [start, end] = [flakeAt(f, 0, BAND), flakeAt(f, SNOW_LOOP_S, BAND)];
      assert.ok(Math.abs(start.x - end.x) < 1e-6 && Math.abs(start.y - end.y) < 1e-6, 'a flake jumps when the loop restarts');
    }
  });

  test('standing still, the flakes spread over the band instead of bunching at its top', () => {
    const ys = snowLayout(14, 7).map((f) => flakeAt(f, 0, BAND).y);
    assert.ok(ys.filter((y) => y > BAND / 2).length >= 3);
    assert.ok(ys.filter((y) => y < BAND / 2).length >= 3);
  });

  test('flakes fade in at the top and out toward the bottom edge', () => {
    for (const f of snowLayout(14, 7))
      for (let t = 0; t < SNOW_LOOP_S; t += 0.5) {
        const p = flakeAt(f, t, BAND);
        assert.ok(p.opacity >= 0 && p.opacity <= f.op);
        if (p.y + f.r >= BAND) assert.equal(p.opacity, 0);
      }
  });
});

describe('snowdrift', () => {
  test('wider than the band by its padding on both sides, with the floor inside the box', () => {
    const d = snowdrift(340);
    assert.equal(d.width, 340 + 2 * d.pad);
    assert.ok(d.floor > 0 && d.floor < d.height);
    assert.ok(d.hill.endsWith('Z') && d.bank.endsWith('Z'));
  });
});
