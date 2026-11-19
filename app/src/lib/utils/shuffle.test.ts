import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { shuffled } from '@/lib/utils/shuffle';

describe('shuffled', () => {
  test('keeps every item once and leaves the input alone', () => {
    const items = [1, 2, 3, 4, 5, 6];
    const out = shuffled(items);
    assert.deepEqual([...out].sort((a, b) => a - b), items);
    assert.deepEqual(items, [1, 2, 3, 4, 5, 6]);
  });

  test('follows the random source', () => {
    assert.deepEqual(shuffled(['a', 'b', 'c'], () => 0), ['b', 'c', 'a']);
    assert.deepEqual(shuffled(['a', 'b', 'c'], () => 0.999), ['a', 'b', 'c']);
  });

  test('empty and single lists', () => {
    assert.deepEqual(shuffled([]), []);
    assert.deepEqual(shuffled(['x']), ['x']);
  });
});
