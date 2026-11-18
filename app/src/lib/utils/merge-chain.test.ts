import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { mergeCellTiles, mergeChain, twinTiles } from '@/lib/utils/merge-chain';

describe('2048 merge chain', () => {
  const chain = mergeChain();
  test('merges speed up, then the last one takes its time', () => {
    assert.equal(chain.merges.length, 9);
    assert.equal(chain.merges[0], 830);
    const gaps = chain.merges.map((m, i) => m - (i ? chain.merges[i - 1] : 500));
    for (let i = 1; i < gaps.length; i++) assert.ok(gaps[i] <= gaps[i - 1]);
    assert.equal(chain.last, chain.merges[8] + 650);
    assert.equal(chain.slam, chain.last + 330);
  });
  test('the merge cell always shows exactly one tile, each replaced by the next', () => {
    const tiles = mergeCellTiles(chain);
    assert.equal(tiles.length, 11);
    for (let i = 1; i < tiles.length; i++) assert.equal(tiles[i].from, tiles[i - 1].to);
    assert.equal(tiles[10].v, 2048);
  });
  test('each twin lands as the next tile is born', () => {
    const tiles = mergeCellTiles(chain);
    const twins = twinTiles(chain);
    assert.equal(twins.length, 10);
    twins.forEach((tw, i) => {
      assert.equal(tw.v, tiles[i].v);
      assert.equal(tw.end, tiles[i + 1].from);
      assert.ok(tw.spawn < tw.start && tw.start < tw.end);
    });
    assert.deepEqual(
      twins.map((tw) => tw.source),
      [0, 1, 2, 3, 0, 1, 2, 3, 0, 0],
    );
  });
});
