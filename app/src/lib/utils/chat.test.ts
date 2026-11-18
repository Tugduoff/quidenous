import assert from 'node:assert/strict';
import { test } from 'node:test';
import { frameSize, insertMention, ratioOf, twoColumns } from '@/lib/utils/chat';

test('ratio: width over height, square when unknown', () => {
  assert.equal(ratioOf(200, 100), 2);
  assert.equal(ratioOf(0, 100), 1);
  assert.equal(ratioOf(100, 0), 1);
});

test('a photo in a bubble: 70 % of the screen, 300 wide and 320 tall at most, 120 wide at least', () => {
  assert.deepEqual(frameSize(400, 300, 360), { width: 252, maxWidth: '100%', aspectRatio: 4 / 3 });
  assert.equal(frameSize(400, 300, 1000).width, 300);
  const tall = frameSize(100, 1000, 400);
  assert.equal(tall.width, 120);
  assert.equal(tall.aspectRatio, 120 / 320);
  assert.equal(frameSize(300, 400, 400).width, 240);
});

test('two columns: each picture under the shorter one', () => {
  const p = (w: number, h: number) => ({ width: w, height: h });
  const [a, b, c, d] = [p(100, 300), p(100, 100), p(100, 100), p(100, 100)];
  assert.deepEqual(twoColumns([a, b, c, d], 100, 6), [[a], [b, c, d]]);
  assert.deepEqual(twoColumns([], 100, 6), [[], []]);
});

test('picking a mention replaces the word being typed, one space after', () => {
  assert.deepEqual(insertMention('hi @lo', { at: 3, query: 'lo' }, '@Louis', 500), { next: 'hi @Louis ', pos: 10 });
  assert.deepEqual(insertMention('hi @louxx there', { at: 3, query: 'lo' }, '@Louis', 500), { next: 'hi @Louis there', pos: 10 });
  assert.deepEqual(insertMention('@l', { at: 0, query: 'l' }, '@Louis', 4), { next: '@Lou', pos: 4 });
});
