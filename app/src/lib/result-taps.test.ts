// Live reactions' pure logic. Run from app/: `bun test src/lib/result-taps.test.ts`.
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import {
  addMine,
  applyLive,
  burstList,
  emptyReactions,
  type LiveTap,
  mergeTaps,
  RESULT_CAP,
  RESULT_REACTIONS,
  type ResultReactions,
  TapBatcher,
  tapsLeft,
  visibleTaps,
} from './result-taps';

const live = (member_id: number, user_id: number, emoji: string, count: number): LiveTap => ({ member_id, user_id, emoji, count });

/** Timers the test fires by hand. */
function manualTimers() {
  const pending = new Map<number, () => void>();
  let id = 0;
  return {
    timers: {
      set: (fn: () => void) => {
        pending.set(++id, fn);
        return id;
      },
      clear: (h: unknown) => void pending.delete(h as number),
    },
    fire: () => {
      const fns = [...pending.values()];
      pending.clear();
      fns.forEach((f) => f());
    },
    get size() {
      return pending.size;
    },
  };
}

describe('palette', () => {
  test('8 emojis, the server order', () => {
    assert.deepEqual([...RESULT_REACTIONS], ['😂', '🔥', '😱', '👀', '💀', '❤️', '👏', '🙈']);
  });
});

describe('mergeTaps', () => {
  test('adds up per emoji, palette order, drops unknown emojis and empty counts', () => {
    assert.deepEqual(
      mergeTaps(
        [
          { emoji: '🙈', count: 1 },
          { emoji: '😂', count: 2 },
          { emoji: '🍕', count: 3 },
        ],
        [
          { emoji: '😂', count: 1 },
          { emoji: '🔥', count: 0 },
        ],
      ),
      [
        { emoji: '😂', count: 3 },
        { emoji: '🙈', count: 1 },
      ],
    );
  });
});

describe('addMine (optimistic)', () => {
  test('counts, my counts and my name', () => {
    let d = emptyReactions(7);
    d = addMine(d, [{ emoji: '🔥', count: 1 }], 5);
    d = addMine(d, [{ emoji: '😂', count: 2 }], 5);
    d = addMine(d, [{ emoji: '🔥', count: 1 }], 5);
    assert.equal(d.mine, 4);
    assert.deepEqual(d.reactions, [
      { emoji: '😂', count: 2, mine: 2, members: [{ member_id: 5, count: 2 }] },
      { emoji: '🔥', count: 2, mine: 2, members: [{ member_id: 5, count: 2 }] },
    ]);
  });

  test('never goes over the cap: the extras are dropped', () => {
    let d: ResultReactions = { ...emptyReactions(1), mine: RESULT_CAP - 2 };
    d = addMine(d, [{ emoji: '😂', count: 5 }], 5);
    assert.equal(d.mine, RESULT_CAP);
    assert.equal(d.reactions[0].count, 2);
    assert.equal(tapsLeft(d), 0);
    assert.equal(addMine(d, [{ emoji: '🔥', count: 1 }], 5), d);
  });

  test("uses the server's cap", () => {
    const d = addMine({ ...emptyReactions(1), cap: 3 }, [{ emoji: '😂', count: 10 }], null);
    assert.equal(d.mine, 3);
    assert.deepEqual(d.reactions[0].members, []);
  });

  test('does not touch the original', () => {
    const d = emptyReactions(1);
    addMine(d, [{ emoji: '😂', count: 1 }], 5);
    assert.deepEqual(d, emptyReactions(1));
  });
});

describe('applyLive', () => {
  const base: ResultReactions = {
    round_id: 3,
    cap: 30,
    mine: 1,
    reactions: [{ emoji: '🔥', count: 2, mine: 1, members: [{ member_id: 5, count: 1 }, { member_id: 8, count: 1 }] }],
  };

  test("adds other people's taps to the counts and the names, not to mine", () => {
    const d = applyLive(base, [live(8, 80, '🔥', 2), live(9, 90, '🔥', 1), live(9, 90, '😂', 3)]);
    assert.equal(d.mine, 1);
    assert.deepEqual(d.reactions, [
      { emoji: '😂', count: 3, mine: 0, members: [{ member_id: 9, count: 3 }] },
      {
        emoji: '🔥',
        count: 5,
        mine: 1,
        members: [
          { member_id: 5, count: 1 },
          { member_id: 8, count: 3 },
          { member_id: 9, count: 1 },
        ],
      },
    ]);
  });

  test('nothing to add: the same counts', () => {
    assert.deepEqual(applyLive(base, []), base);
  });
});

describe('visibleTaps', () => {
  const taps = [live(5, 50, '😂', 1), live(8, 80, '🔥', 2), live(9, 90, '👀', 1), live(10, 100, '🍕', 1), live(11, 110, '😂', 0)];

  test('skips mine, the people I blocked, unknown emojis and empty counts', () => {
    assert.deepEqual(visibleTaps(taps, 5, new Set([90])), [live(8, 80, '🔥', 2)]);
  });

  test('the big screen (nobody, nothing blocked) sees every valid tap', () => {
    assert.equal(visibleTaps(taps, null, new Set()).length, 3);
  });
});

describe('burstList', () => {
  test('interleaves the emojis and stops at the pool size', () => {
    const list = burstList(
      [
        { emoji: '😂', count: 3 },
        { emoji: '🔥', count: 1 },
        { emoji: '🍕', count: 4 },
      ],
      3,
    );
    assert.deepEqual(list, ['😂', '🔥', '😂']);
    assert.equal(burstList([{ emoji: '😂', count: 100 }], 24).length, 24);
    assert.deepEqual(burstList([], 24), []);
  });
});

describe('TapBatcher', () => {
  test('taps within the delay go in one request, merged', async () => {
    const sent: unknown[] = [];
    const clock = manualTimers();
    const b = new TapBatcher(async (taps) => void sent.push(taps), 250, clock.timers);
    b.add('🔥');
    b.add('😂');
    b.add('🔥');
    b.add('🍕');
    assert.equal(clock.size, 1, 'one timer for the whole batch');
    assert.deepEqual(b.queued, [
      { emoji: '😂', count: 1 },
      { emoji: '🔥', count: 2 },
    ]);
    clock.fire();
    await Promise.resolve();
    assert.deepEqual(sent, [
      [
        { emoji: '😂', count: 1 },
        { emoji: '🔥', count: 2 },
      ],
    ]);
    assert.deepEqual(b.queued, []);
    assert.ok(b.idle);
  });

  test('counts the requests in flight; later taps wait for the next one', async () => {
    let answer!: () => void;
    const sent: unknown[] = [];
    const clock = manualTimers();
    const b = new TapBatcher(
      (taps) => {
        sent.push(taps);
        return new Promise<void>((r) => (answer = r));
      },
      250,
      clock.timers,
    );
    b.add('😂');
    const first = b.flush();
    assert.equal(b.inFlight, 1);
    b.add('👀');
    assert.ok(!b.idle);
    assert.deepEqual(b.queued, [{ emoji: '👀', count: 1 }]);
    answer();
    await first;
    assert.equal(b.inFlight, 0);
    clock.fire();
    await Promise.resolve();
    assert.equal(sent.length, 2);
  });

  test('a failed request still frees its slot', async () => {
    const b = new TapBatcher(() => Promise.reject(new Error('down')), 250, manualTimers().timers);
    b.add('😂');
    await assert.rejects(b.flush());
    assert.equal(b.inFlight, 0);
  });

  test('nothing queued: no request', async () => {
    let calls = 0;
    const b = new TapBatcher(async () => void calls++, 250, manualTimers().timers);
    await b.flush();
    assert.equal(calls, 0);
  });
});
