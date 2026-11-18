import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { boardLayout, geometry, hit, itemsWidth, measured, nextToWire, PAD, plugEnds, rowTop, verdict, wirePath } from '@/lib/utils/choose';

describe('the wire board', () => {
  test('rows get compact as the group grows', () => {
    assert.equal(geometry(6).row, 54);
    assert.equal(geometry(7).row, 46);
    assert.equal(geometry(13).row, 40);
  });

  test('the items column fits its longest word, on two lines at most', () => {
    assert.equal(itemsWidth([]), 28);
    assert.equal(itemsWidth(['ab', 'chocolat']), 8 * 9 + 28);
    assert.equal(itemsWidth(['un deux trois quatre']), 10 * 9 + 28);
  });

  test('measured: all of the first count entries, or nothing', () => {
    assert.deepEqual(measured([1, 2, 3], 2), [1, 2]);
    assert.equal(measured([1, undefined, 3], 3), null);
    assert.equal(measured([1], 2), null);
    assert.deepEqual(measured([], 0), []);
  });

  test('the layout keeps the items between a quarter and a third, the wires a fifth', () => {
    const l = boardLayout(400, 3, ['a'], 120, 1000);
    assert.equal(l.itemsW, 96);
    assert.equal(l.itemsX, 400 - PAD - 96);
    assert.equal(l.left, Math.round(l.itemsX - 80 + PAD));
    assert.equal(l.content, PAD * 2 + 3 * 54 + 2 * 8);
    assert.equal(l.height, Math.max(140, Math.min(l.content, 1000)));
    assert.equal(l.stackY, (l.height - 120) / 2);
    assert.equal(boardLayout(400, 3, ['x'.repeat(80)], 120, 1000).itemsW, 132);
  });

  test('the board scrolls past the room it has, never shorter than the items', () => {
    assert.equal(boardLayout(400, 30, ['a'], 100, 300).height, 300);
    assert.equal(boardLayout(400, 30, ['a'], 100, 100).height, 260);
    assert.equal(boardLayout(400, 30, ['a'], 400, 100).height, 400 + PAD * 2);
  });

  test('row tops', () => {
    const g = geometry(2);
    assert.equal(rowTop(g, 0), PAD);
    assert.equal(rowTop(g, 2), PAD + 2 * (g.row + g.gap));
  });

  test('a dropped wire plugs into the closest item once near the column', () => {
    const b = { itemsX: 200, rects: [{ y: 0, h: 40 }, { y: 50, h: 40 }] };
    assert.equal(hit(100, 10, b), -1);
    assert.equal(hit(160, 10, b), 0);
    assert.equal(hit(160, 80, b), 1);
    assert.equal(hit(300, 80, { itemsX: 200, rects: [] }), -1);
  });

  test('several wires into one item spread along its edge', () => {
    const people = [{ id: 1 }, { id: 2 }, { id: 3 }];
    const ends = plugEnds([{ y: 0, h: 60 }, { y: 100, h: 40 }], people, { 1: 0, 2: 0, 3: 1 }, 50);
    assert.deepEqual(ends, { 1: { x: 50, y: 10 }, 2: { x: 50, y: 50 }, 3: { x: 50, y: 120 } });
  });

  test('tapping on: the next one still to wire, wrapping around', () => {
    const people = [{ id: 1 }, { id: 2 }, { id: 3 }];
    assert.equal(nextToWire(people, 0, 1, { 2: 0 })?.id, 3);
    assert.equal(nextToWire(people, 2, 3, {})?.id, 1);
    assert.equal(nextToWire(people, 1, 2, { 1: 0, 3: 0 }), undefined);
  });

  test('a wire path starts and ends where asked', () => {
    const d = wirePath(0, 0, 100, 50);
    assert.ok(d.startsWith('M 0 0 C '));
    assert.ok(d.endsWith(' 100 50'));
  });
});

test('verdict: the group saw it coming, or not', () => {
  const p = { member_id: 1, guesses: [] };
  assert.equal(verdict({ ...p, real: null, top: [1] }), 'noAnswer');
  assert.equal(verdict({ ...p, real: 1, top: [] }), 'noGuess');
  assert.equal(verdict({ ...p, real: 1, top: [0, 1] }), 'match');
  assert.equal(verdict({ ...p, real: 2, top: [0, 1] }), 'surprise');
});
