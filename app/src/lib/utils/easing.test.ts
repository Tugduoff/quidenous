import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { backOut, clamp, clamp01, easeInOutCubic, easeOutCubic, noise } from '@/lib/utils/easing';

describe('easing', () => {
  test('clamp', () => {
    assert.equal(clamp(-1, 0, 5), 0);
    assert.equal(clamp(3, 0, 5), 3);
    assert.equal(clamp(9, 0, 5), 5);
  });
  test('clamp01', () => {
    assert.equal(clamp01(-2), 0);
    assert.equal(clamp01(0.3), 0.3);
    assert.equal(clamp01(4), 1);
  });
  test('curves start at 0 and end at 1', () => {
    for (const f of [easeOutCubic, easeInOutCubic, (x: number) => backOut(x, 1.4)]) {
      assert.equal(f(0), 0);
      assert.equal(f(1), 1);
    }
    assert.equal(easeInOutCubic(0.5), 0.5);
  });
  test('backOut overshoots past 1 before settling', () => {
    assert.ok(backOut(0.8, 1.4) > 1);
    assert.ok(backOut(0.8, 0) <= 1);
  });
  test('noise is deterministic and in [0, 1)', () => {
    for (let i = 0; i < 100; i++) {
      const v = noise(i * 1.7);
      assert.equal(v, noise(i * 1.7));
      assert.ok(v >= 0 && v < 1);
    }
  });
});
