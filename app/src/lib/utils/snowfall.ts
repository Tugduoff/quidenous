import { makeRand } from '@/lib/utils/rand';

export const SNOW_LOOP_S = 120;

type Depth = 0 | 1 | 2;
type Range = readonly [number, number];
type DepthLook = { radius: Range; fallSeconds: readonly number[]; sway: Range; gust: number; opacity: Range };

const DEPTHS: readonly [DepthLook, DepthLook, DepthLook] = [
  { radius: [1.1, 1.6], fallSeconds: [40, 30], sway: [2, 4], gust: 3, opacity: [0.34, 0.5] },
  { radius: [2, 2.7], fallSeconds: [24, 20], sway: [4, 7], gust: 7, opacity: [0.6, 0.78] },
  { radius: [8, 10.5], fallSeconds: [20, 15], sway: [7, 11], gust: 12, opacity: [0.8, 0.92] },
];
export const CRYSTAL_DEPTH: Depth = 2;
export const SPECK_DEPTH: Depth = 0;
const SWAY_PERIODS_S = [5, 6, 8, 10];
const SPIN_PERIODS_S = [40, -60, 60];
const GUST_PERIOD_S = 24;
export const SNOW_PERIODS_S = [...DEPTHS.flatMap((d) => d.fallSeconds), ...SWAY_PERIODS_S, ...SPIN_PERIODS_S.map(Math.abs), GUST_PERIOD_S];

const GOLDEN = 0.618034;
const NEAR_SHARE = 0.2;
const MID_SHARE = 0.36;
const MIN_PER_NEAR_DEPTH = 2;
const COLUMN_JITTER = 0.7;
const FADE_IN_PX = 14;
const FADE_OUT_SHARE = 0.42;

export const SNOW_BOOST = { dark: 1, light: 1.45 } as const;

export type Flake = {
  depth: Depth;
  left: number;
  r: number;
  fall: number;
  swayAmp: number;
  sway: number;
  gust: number;
  phase: number;
  swayPhase: number;
  spin: number;
  op: number;
};

export function depthCounts(count: number): [number, number, number] {
  const near = Math.max(MIN_PER_NEAR_DEPTH, Math.round(count * NEAR_SHARE));
  const mid = Math.max(MIN_PER_NEAR_DEPTH, Math.round(count * MID_SHARE));
  return [Math.max(0, count - near - mid), mid, near];
}

export function snowLayout(count: number, seed: number): Flake[] {
  const rand = makeRand(seed);
  const between = ([lo, hi]: Range) => rand.range(lo, hi);
  const flakes: Flake[] = [];
  depthCounts(count).forEach((n, depth) => {
    const look = DEPTHS[depth];
    const columnShift = rand.f();
    const firstPhase = rand.f();
    for (let i = 0; i < n; i++) {
      flakes.push({
        depth: depth as Depth,
        left: ((i + columnShift + (1 - COLUMN_JITTER) / 2 + rand.f() * COLUMN_JITTER) / n) % 1,
        r: between(look.radius),
        fall: rand.pick(look.fallSeconds),
        swayAmp: between(look.sway),
        sway: rand.pick(SWAY_PERIODS_S),
        gust: look.gust,
        phase: (firstPhase + i * GOLDEN) % 1,
        swayPhase: rand.f() * Math.PI * 2,
        spin: SPIN_PERIODS_S[i % SPIN_PERIODS_S.length],
        op: between(look.opacity),
      });
    }
  });
  return flakes;
}

const CRYSTAL_ARM = 10.4;
const BARB_BASE = 5.4;
const BARB_TIP = 9;
const BARB_ANGLE = 0.34;

export const CRYSTAL = (() => {
  const at = (angle: number, distance: number) => `${(12 + distance * Math.cos(angle)).toFixed(2)} ${(12 + distance * Math.sin(angle)).toFixed(2)}`;
  let d = '';
  for (let k = 0; k < 6; k++) {
    const angle = (k * Math.PI) / 3 - Math.PI / 2;
    const arm = `M12 12L${at(angle, CRYSTAL_ARM)}`;
    const barbs = `M${at(angle - BARB_ANGLE, BARB_TIP)}L${at(angle, BARB_BASE)}L${at(angle + BARB_ANGLE, BARB_TIP)}`;
    d += arm + barbs;
  }
  return d;
})();

export function flakeAt(f: Flake, t: number, height: number) {
  'worklet';
  const span = height + 4 * f.r;
  const y = ((t / f.fall + f.phase) % 1) * span - 2 * f.r;
  const x = f.swayAmp * Math.sin((2 * Math.PI * t) / f.sway + f.swayPhase) + f.gust * Math.sin((2 * Math.PI * t) / GUST_PERIOD_S);
  const fadeIn = Math.min(1, (y + 2 * f.r) / FADE_IN_PX);
  const fadeOut = Math.max(0, Math.min(1, (height - y - f.r) / (FADE_OUT_SHARE * height)));
  return { x, y, opacity: f.op * fadeIn * fadeOut, turn: f.depth === CRYSTAL_DEPTH ? (360 * t) / f.spin + f.phase * 360 : 0 };
}
