import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { WrappedCard } from '@/lib/api/wrapped';
import { bigSize, colorsOf } from '@/lib/utils/wrapped';

const card = (people: { color: string }[]) => ({ people }) as unknown as WrappedCard;

describe('wrapped card colours', () => {
  test('nobody on the card: the accent pairs in turn', () => {
    assert.deepEqual(colorsOf(card([]), 0), ['#C8FF3D', '#5EF2B0']);
    assert.deepEqual(colorsOf(card([]), 7), ['#FF6DB5', '#FFB547']);
  });
  test('one person: their colour, then the pair', () => {
    assert.deepEqual(colorsOf(card([{ color: '#111111' }]), 0), ['#111111', '#5EF2B0']);
  });
  test('two people: both colours', () => {
    assert.deepEqual(colorsOf(card([{ color: '#111111' }, { color: '#222222' }]), 3), ['#111111', '#222222']);
  });
});

describe('the big figure size', () => {
  test('steps by length', () => {
    assert.equal(bigSize('42'), 116);
    assert.equal(bigSize('12 345'), 88);
    assert.equal(bigSize('9 messages'), 53);
  });
  test('a long figure fits its longest word', () => {
    assert.equal(bigSize('le plus souvent premier'), 38);
    assert.equal(bigSize('incontestablement champion'), 31);
  });
});
