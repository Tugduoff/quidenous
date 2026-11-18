// « Qui a dit ça ? » logic tests. Run from app/: `bun test src/lib/said.test.ts`.
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import type { SaidView } from '@/lib/api/client';
import {
  guessGroups,
  holderOf,
  initMatch,
  isComplete,
  type Match,
  type MatchAction,
  matchReducer,
  nextOpen,
  pickLine,
  places,
  placed,
  revealItems,
  suspects,
  toGuess,
  toGuesses,
  topScorers,
} from '@/lib/utils/said';

const run = (s: Match, ...actions: MatchAction[]) => actions.reduce(matchReducer, s);

describe('what there is to guess', () => {
  const said: Pick<SaidView, 'answers' | 'my_answer' | 'writer_ids'> = {
    writer_ids: [1, 2, 3, 4],
    my_answer: { id: 70, text: 'le mien' },
    answers: [
      { id: 90, text: 'une poêle', hidden: false },
      { id: 70, text: 'le mien', hidden: false },
      { id: 20, text: null, hidden: true },
      { id: 50, text: null, hidden: false },
    ],
  };
  test('every answer but mine and passes; a hidden one still needs a guess', () => {
    assert.deepEqual(
      toGuess(said).map((a) => a.id),
      [90, 20],
    );
  });
  test('no answers yet (writing): nothing', () => {
    assert.deepEqual(toGuess({ answers: null, my_answer: null }), []);
  });
  test('the writers but me, in member order', () => {
    assert.deepEqual(suspects(said, 2), [1, 3, 4]);
    assert.deepEqual(suspects(said, 9), [1, 2, 3, 4]);
  });
});

describe('the matching game', () => {
  const fresh = () => initMatch([10, 20, 30]);

  test('starts on the first answer', () => {
    assert.deepEqual(fresh(), { order: [10, 20, 30], picks: {}, sel: 10 });
  });

  test('tapping faces one after the other fills the answers in order', () => {
    const s = run(fresh(), { type: 'pick', member: 1 }, { type: 'pick', member: 2 }, { type: 'pick', member: 3 });
    assert.deepEqual(s.picks, { 10: 1, 20: 2, 30: 3 });
    assert.equal(s.sel, null);
    assert.ok(isComplete(s));
    assert.deepEqual(toGuesses(s), [
      { answer_id: 10, member_id: 1 },
      { answer_id: 20, member_id: 2 },
      { answer_id: 30, member_id: 3 },
    ]);
  });

  test('after a pick, the next open answer waits (going round)', () => {
    const s = run(fresh(), { type: 'select', answer: 30 }, { type: 'pick', member: 1 });
    assert.equal(s.sel, 10);
    assert.equal(nextOpen(s.order, s.picks, 30), 10);
  });

  test('a person already used swaps with the one there', () => {
    let s = run(fresh(), { type: 'pick', member: 1 }, { type: 'pick', member: 2 });
    // 10 → 1, 20 → 2; now put 1 on 20.
    s = run(s, { type: 'select', answer: 20 }, { type: 'pick', member: 1 });
    assert.deepEqual(s.picks, { 10: 2, 20: 1 });
    // One person per answer, never twice.
    assert.equal(new Set(Object.values(s.picks)).size, Object.keys(s.picks).length);
  });

  test('a person used elsewhere moves to an empty answer (their old one opens up)', () => {
    let s = run(fresh(), { type: 'pick', member: 1 });
    s = run(s, { type: 'select', answer: 30 }, { type: 'pick', member: 1 });
    assert.deepEqual(s.picks, { 30: 1 });
    assert.equal(s.sel, 10);
  });

  test('the same face again takes it off', () => {
    const s = run(fresh(), { type: 'pick', member: 1 }, { type: 'select', answer: 10 }, { type: 'pick', member: 1 });
    assert.deepEqual(s.picks, {});
    assert.equal(s.sel, 10);
  });

  test('tapping the selected answer again unselects it; a face then shows where it is', () => {
    let s = run(fresh(), { type: 'pick', member: 1 }, { type: 'pick', member: 2 }, { type: 'pick', member: 3 });
    assert.equal(s.sel, null);
    s = run(s, { type: 'pick', member: 2 });
    assert.equal(s.sel, 20);
    s = run(s, { type: 'select', answer: 20 });
    assert.equal(s.sel, null);
    // Someone on no answer, nothing waiting: nothing happens.
    assert.equal(run(s, { type: 'pick', member: 9 }), s);
  });

  test('clear and reset', () => {
    let s = run(fresh(), { type: 'pick', member: 1 }, { type: 'pick', member: 2 });
    s = run(s, { type: 'clear', answer: 10 });
    assert.deepEqual(s.picks, { 20: 2 });
    assert.equal(s.sel, 10);
    assert.equal(placed(s), 1);
    assert.ok(!isComplete(s));
    assert.deepEqual(run(s, { type: 'reset' }), fresh());
  });

  test('an unknown answer cannot be selected; picks of answers not shown are dropped', () => {
    const s = fresh();
    assert.equal(run(s, { type: 'select', answer: 99 }), s);
    assert.deepEqual(initMatch([10], { 10: 1, 99: 2 }).picks, { 10: 1 });
    assert.equal(holderOf({ 10: 1, 20: 2 }, 2), 20);
    assert.equal(holderOf({ 10: 1 }, 2), null);
  });

  test('nothing to place is never complete', () => {
    assert.ok(!isComplete(initMatch([])));
  });
});

describe('the reveal', () => {
  const said: Pick<SaidView, 'answers' | 'my_answer' | 'my_guesses' | 'result'> = {
    answers: [
      { id: 7, text: 'b', hidden: false },
      { id: 3, text: 'a', hidden: false },
      { id: 5, text: null, hidden: true },
    ],
    my_answer: { id: 3, text: 'a' },
    my_guesses: [
      { answer_id: 7, member_id: 2 },
      { answer_id: 5, member_id: 4 },
    ],
    result: {
      answers: [
        {
          id: 7,
          author_id: 2,
          found: 2,
          fooled: 1,
          guesses: [
            { voter_id: 1, member_id: 2 },
            { voter_id: 3, member_id: 2 },
            { voter_id: 4, member_id: 3 },
          ],
        },
        {
          id: 3,
          author_id: 1,
          found: 1,
          fooled: 2,
          guesses: [
            { voter_id: 2, member_id: 4 },
            { voter_id: 3, member_id: 1 },
            { voter_id: 4, member_id: 2 },
          ],
        },
        {
          id: 5,
          author_id: 4,
          found: 0,
          fooled: 3,
          guesses: [
            { voter_id: 1, member_id: 3 },
            { voter_id: 2, member_id: 3 },
            { voter_id: 3, member_id: 2 },
          ],
        },
      ],
      scores: [
        { member_id: 4, points: 3, found: 0, fooled: 3 },
        { member_id: 1, points: 3, found: 1, fooled: 2 },
        { member_id: 2, points: 1, found: 0, fooled: 1 },
        { member_id: 3, points: 2, found: 2, fooled: 0 },
      ],
    },
  };

  test("keeps the server's order (most found first, best hidden last), with texts and my guesses", () => {
    const items = revealItems(said, 1);
    assert.deepEqual(
      items.map((i) => [i.id, i.text, i.hidden, i.own, i.myGuess]),
      [
        [7, 'b', false, false, 2],
        [3, 'a', false, true, null],
        [5, null, true, false, 4],
      ],
    );
  });

  test('an answer missing from the list still reveals its author (no text)', () => {
    const items = revealItems({ ...said, answers: null }, 1);
    assert.equal(items.length, 3);
    assert.equal(items[0].text, null);
  });

  test('no result: nothing to reveal', () => {
    assert.deepEqual(revealItems({ ...said, result: null }, 1), []);
  });

  test('guesses grouped by the person named, most named first, the author marked', () => {
    assert.deepEqual(guessGroups(said.result!.answers[0]), [
      { member_id: 2, voters: [1, 3], right: true },
      { member_id: 3, voters: [4], right: false },
    ]);
    assert.deepEqual(guessGroups({ author_id: 1, guesses: [] }), []);
  });

  test('places: same points share a place', () => {
    assert.deepEqual(places(said.result!.scores), [1, 1, 4, 3]);
    assert.deepEqual(places([{ points: 3 }, { points: 2 }, { points: 2 }, { points: 0 }]), [1, 2, 2, 4]);
  });

  test('winners: the top scorers, nobody when nobody scored', () => {
    assert.deepEqual(topScorers(said.result!.scores), [4, 1]);
    assert.deepEqual(topScorers([{ member_id: 1, points: 0, found: 0, fooled: 0 }]), []);
    assert.deepEqual(topScorers([]), []);
  });
});

test('pickLine is stable for a seed and always in range', () => {
  const lines = ['a', 'b', 'c'];
  for (let seed = -5; seed < 200; seed++) {
    assert.ok(lines.includes(pickLine(lines, seed)));
    assert.equal(pickLine(lines, seed), pickLine(lines, seed));
  }
  assert.ok(new Set(Array.from({ length: 30 }, (_, i) => pickLine(lines, i))).size > 1);
});
