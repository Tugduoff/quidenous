import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { ballistic } from '@/lib/utils/ballistics';

const close = (a: number, b: number, eps = 1e-3) => assert.ok(Math.abs(a - b) < eps, `${a} ≉ ${b}`);

describe('ballistic flight', () => {
  test('starts where and how it was thrown', () => {
    const p = ballistic(10, 20, 30, -40, 900, 1.6, 0);
    assert.deepEqual(p, { x: 10, y: 20, vx: 30, vy: -40 });
  });
  test('with almost no drag it is the textbook parabola', () => {
    const p = ballistic(0, 0, 100, -200, 50, 1e-6, 2);
    close(p.x, 200, 0.01);
    close(p.y, -400 + 0.5 * 50 * 4, 0.01);
    close(p.vy, -200 + 50 * 2, 0.01);
  });
  test('drag brings it to terminal velocity g / k', () => {
    const p = ballistic(0, 0, 300, 0, 900, 2, 30);
    close(p.vx, 0);
    close(p.vy, 450);
  });
});
