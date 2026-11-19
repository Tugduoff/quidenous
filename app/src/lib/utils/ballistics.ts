export function ballistic(x: number, y: number, vx: number, vy: number, g: number, k: number, t: number) {
  'worklet';
  const decay = Math.exp(-k * t);
  const e = 1 - decay;
  const term = g / k;
  return {
    x: x + (vx / k) * e,
    y: y + term * t + ((vy - term) / k) * e,
    vx: vx * decay,
    vy: term + (vy - term) * decay,
  };
}
