/** Seeded random (mulberry32): the same milestone always throws the same confetti, so captures are reproducible. */
export function makeRand(seed: number) {
  let a = seed >>> 0 || 1;
  const f = () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return {
    f,
    range: (lo: number, hi: number) => lo + f() * (hi - lo),
    pick: <T,>(xs: readonly T[]) => xs[Math.floor(f() * xs.length)],
    sign: () => (f() < 0.5 ? -1 : 1),
  };
}
export type Rand = ReturnType<typeof makeRand>;
