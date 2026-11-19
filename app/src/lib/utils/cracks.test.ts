import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { cracks } from '@/lib/utils/cracks';
import { makeRand } from '@/lib/utils/rand';

describe('screen cracks', () => {
  const segs = cracks(makeRand(666), 200, 300, 1100, 400, 800);
  test('the same seed cracks the same way', () => {
    assert.deepEqual(cracks(makeRand(666), 200, 300, 1100, 400, 800), segs);
  });
  test('nothing is drawn before the impact, and each piece takes a little time', () => {
    assert.ok(segs.length > 9);
    for (const s of segs) {
      assert.ok(s.at >= 1100 - 60, `starts at ${s.at}`);
      assert.ok(s.dur >= 18);
      assert.ok(s.thick >= 1.2 && s.thick <= 3);
    }
  });
  test('the first piece of each crack starts next to the impact', () => {
    const first = segs.filter((s) => Math.hypot(s.x - 200, s.y - 300) < 10.001);
    assert.equal(first.length, 9);
  });
});
