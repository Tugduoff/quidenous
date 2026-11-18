import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { clip, mergeQuestions, questionKey } from '@/lib/utils/pack-draft';

describe('merging questions into a pack draft', () => {
  test('skips duplicates (case-insensitive), too short and too long ones', () => {
    const list = [{ text: 'Qui ronfle le plus ?', kind: 'pick' as const, anonymous: false }];
    const out = mergeQuestions(list, [
      { text: '  qui RONFLE le plus ?  ', kind: 'pick' },
      { text: 'Court', kind: 'pick' },
      { text: 'x'.repeat(201), kind: 'pick' },
      { text: ' Qui cuisine le mieux ? ', kind: 'rank', anonymous: true },
    ]);
    assert.deepEqual(out, [list[0], { text: 'Qui cuisine le mieux ?', kind: 'rank', anonymous: true }]);
  });
  test('a choose question keeps its trimmed items, or is dropped with bad ones', () => {
    const out = mergeQuestions(
      [],
      [
        { text: 'Plutôt mer ou montagne ?', kind: 'choose', items: [' mer ', 'montagne'] },
        { text: 'Plutôt chat ou chien ?', kind: 'choose', items: ['chat'] },
      ],
    );
    assert.deepEqual(out, [{ text: 'Plutôt mer ou montagne ?', kind: 'choose', items: ['mer', 'montagne'] }]);
  });
  test('the same question twice in the new ones is kept once', () => {
    const out = mergeQuestions([], [
      { text: 'Qui part en retard ?', kind: 'pick' },
      { text: 'qui part en retard ?', kind: 'rank' },
    ]);
    assert.equal(out.length, 1);
  });
});

test('clip keeps whole characters', () => {
  assert.equal(clip('  🎉🎉🎉 fête ', 2), '🎉🎉');
  assert.equal(questionKey('  Hé Toi '), 'hé toi');
});
