import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { activeSkin, inRange, overrideSkin, parseOverride, pickSkin, seasonTag } from '@/lib/utils/season';

const WINTER = { id: 'winter', from: '12-01', until: '01-06' };
const day = (y: number, m: number, d: number, h = 12) => new Date(y, m - 1, d, h);

describe('activeSkin', () => {
  test('wraps the year end, both ends included', () => {
    assert.equal(activeSkin([WINTER], day(2026, 12, 1))?.id, 'winter');
    assert.equal(activeSkin([WINTER], day(2026, 12, 25))?.id, 'winter');
    assert.equal(activeSkin([WINTER], day(2026, 12, 31))?.id, 'winter');
    assert.equal(activeSkin([WINTER], day(2027, 1, 1))?.id, 'winter');
    assert.equal(activeSkin([WINTER], day(2027, 1, 6))?.id, 'winter');
    assert.equal(activeSkin([WINTER], day(2027, 1, 7)), null);
    assert.equal(activeSkin([WINTER], day(2026, 11, 30)), null);
    assert.equal(activeSkin([WINTER], day(2026, 7, 14)), null);
  });

  test('uses the local day, from the first minute to the last', () => {
    assert.equal(activeSkin([WINTER], new Date(2026, 11, 1, 0, 0, 1))?.id, 'winter');
    assert.equal(activeSkin([WINTER], new Date(2026, 10, 30, 23, 59, 59)), null);
    assert.equal(activeSkin([WINTER], new Date(2027, 0, 6, 23, 59, 59))?.id, 'winter');
  });

  test('a range inside one year, and a single day', () => {
    assert.equal(activeSkin([{ id: 'winter', from: '02-10', until: '02-20' }], day(2027, 2, 10))?.id, 'winter');
    assert.equal(activeSkin([{ id: 'winter', from: '02-10', until: '02-20' }], day(2027, 2, 21)), null);
    assert.equal(activeSkin([{ id: 'winter', from: '02-10', until: '02-20' }], day(2027, 1, 15)), null);
    assert.ok(inRange('03-05', '03-05', day(2027, 3, 5)));
    assert.ok(!inRange('03-05', '03-05', day(2027, 3, 6)));
  });

  test('ignores the ids it does not know and malformed entries', () => {
    const d = day(2026, 12, 20);
    assert.equal(activeSkin([{ id: 'halloween', from: '12-01', until: '12-31' }], d), null);
    assert.equal(activeSkin([{ id: 'halloween', from: '12-01', until: '12-31' }, WINTER], d)?.id, 'winter');
    assert.equal(activeSkin([{ id: 'toString', from: '01-01', until: '12-31' }], d), null);
    for (const bad of [{ ...WINTER, from: '13-01' }, { ...WINTER, until: '1-6' }, { ...WINTER, from: undefined }, null, 'winter'])
      assert.equal(activeSkin([bad], d), null, JSON.stringify(bad));
    assert.equal(activeSkin(undefined, d), null);
    assert.equal(activeSkin({ winter: WINTER }, d), null);
  });

  test('the dates come back with the skin', () => {
    assert.deepEqual(activeSkin([WINTER], day(2026, 12, 2)), WINTER);
  });
});

describe('pickSkin', () => {
  test('the phone date decides, not the server active', () => {
    assert.equal(pickSkin({ active: null, skins: [WINTER] }, day(2026, 12, 2))?.id, 'winter');
    assert.equal(pickSkin({ active: 'winter', skins: [WINTER] }, day(2026, 11, 30)), null);
  });
  test('falls back on `active` without a skin list', () => {
    assert.equal(pickSkin({ active: 'winter' }, day(2026, 6, 1))?.id, 'winter');
    assert.equal(pickSkin({ active: 'halloween' }, day(2026, 6, 1)), null);
    assert.equal(pickSkin(null, day(2026, 6, 1)), null);
  });
});

describe('parseOverride', () => {
  test('forces a known skin or none', () => {
    assert.equal(parseOverride('?season=winter'), 'winter');
    assert.equal(parseOverride('?room=3&season=off'), 'off');
    assert.equal(parseOverride('?season=halloween'), null);
    assert.equal(parseOverride(''), null);
  });

  test('a forced skin comes with its default dates, off with none', () => {
    assert.deepEqual(overrideSkin('winter'), WINTER);
    assert.equal(overrideSkin('off'), null);
  });
});

describe('seasonTag', () => {
  test('one tag per run of a season, across the year end', () => {
    assert.equal(seasonTag(WINTER as never, day(2026, 12, 1)), 'winter-2026');
    assert.equal(seasonTag(WINTER as never, day(2026, 12, 31)), 'winter-2026');
    assert.equal(seasonTag(WINTER as never, day(2027, 1, 6)), 'winter-2026');
    assert.equal(seasonTag(WINTER as never, day(2027, 12, 3)), 'winter-2027');
  });
});
