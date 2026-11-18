import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { raysPath, sectorPath, wedgePath } from '@/lib/utils/svg-paths';

describe('svg paths', () => {
  test('a wedge pointing right is symmetric around the horizontal', () => {
    assert.equal(wedgePath(10, 0, Math.PI / 2), `M10,10 L${10 + Math.cos(-Math.PI / 2) * 10},0 L${10 + Math.cos(Math.PI / 2) * 10},20 Z`);
  });
  test('rays: one closed wedge per ray', () => {
    const d = raysPath(100, 14);
    assert.equal(d.match(/M50,50/g)?.length, 14);
    assert.equal(d.match(/Z /g)?.length, 14);
  });
  test('sector: four corners rounded to 2 decimals', () => {
    assert.equal(sectorPath(0, 80, -9, 9), 'M0.00,0.00 L79.02,-12.51 A80,80 0 0 1 79.02,12.51 L0.00,0.00 A0,0 0 0 0 0.00,0.00 Z');
  });
});
