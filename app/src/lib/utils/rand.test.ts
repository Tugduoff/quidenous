import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { makeRand } from '@/lib/utils/rand';

describe('seeded random', () => {
  test('the same seed gives the same sequence', () => {
    const a = makeRand(42);
    const b = makeRand(42);
    for (let i = 0; i < 50; i++) assert.equal(a.f(), b.f());
  });
  test('different seeds diverge', () => {
    assert.notEqual(makeRand(1).f(), makeRand(2).f());
  });
  test('seed 0 behaves like seed 1', () => {
    assert.equal(makeRand(0).f(), makeRand(1).f());
  });
  test('range, pick and sign stay in bounds', () => {
    const r = makeRand(7);
    for (let i = 0; i < 200; i++) {
      const v = r.range(-3, 5);
      assert.ok(v >= -3 && v < 5);
      assert.ok(['a', 'b', 'c'].includes(r.pick(['a', 'b', 'c'])));
      assert.ok([-1, 1].includes(r.sign()));
    }
  });
});
