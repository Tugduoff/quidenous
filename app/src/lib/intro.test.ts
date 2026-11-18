// Intro gating and demo round. Run from app/: `bun test src/lib/intro.test.ts`.
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { demoTally, introAnswer, introPlan, introPreview, readMark } from '@/lib/intro';
import { CAST } from '@/lib/mascot';

describe('intro gating', () => {
  test('a remembered answer is never asked again', () => {
    assert.equal(introPlan(readMark('no')), 'none');
    assert.equal(introPlan(readMark('done')), 'resend');
    assert.equal(introPlan(readMark('skipped')), 'resend');
  });

  test('nothing remembered (or garbage): ask', () => {
    assert.equal(introPlan(readMark(null)), 'ask');
    assert.equal(introPlan(readMark('1')), 'ask');
  });

  test('shown only when the server has it and says so', () => {
    assert.deepEqual(introAnswer(['auth', 'intro'], true), { show: true, mark: null });
    assert.deepEqual(introAnswer(['auth', 'intro'], false), { show: false, mark: 'no' });
  });

  test('an older server: never, and remembered', () => {
    assert.deepEqual(introAnswer(['auth'], null), { show: false, mark: 'no' });
  });

  test('offline or no answer: not shown, nothing remembered', () => {
    assert.deepEqual(introAnswer(null, null), { show: false, mark: null });
    assert.deepEqual(introAnswer(['intro'], null), { show: false, mark: null });
  });

  test('the preview parameter', () => {
    assert.equal(introPreview('?intro=1'), true);
    assert.equal(introPreview('?fete=666&intro=1'), true);
    assert.equal(introPreview('?intro=0'), false);
    assert.equal(introPreview(''), false);
  });
});

describe('demo round', () => {
  test('six ballots, Bulle always wins', () => {
    for (const pick of CAST) {
      const rows = demoTally(pick);
      assert.equal(
        rows.reduce((n, r) => n + r.voters.length, 0),
        6,
      );
      assert.equal(rows[0].who, 'bulle');
      assert.deepEqual(
        rows.filter((r) => r.win).map((r) => r.who),
        ['bulle'],
      );
      assert.ok(rows.some((r) => r.voters.includes('me') && r.who === pick));
    }
  });

  test('percentages and order', () => {
    assert.deepEqual(
      demoTally('bulle').map((r) => [r.who, r.pct]),
      [
        ['bulle', 83],
        ['pave', 17],
      ],
    );
    assert.deepEqual(
      demoTally('roupi').map((r) => [r.who, r.pct]),
      [
        ['bulle', 67],
        ['pave', 17],
        ['roupi', 17],
      ],
    );
    assert.deepEqual(demoTally('pave')[1], { who: 'pave', voters: ['me', 'bulle'], pct: 33, win: false });
  });
});
