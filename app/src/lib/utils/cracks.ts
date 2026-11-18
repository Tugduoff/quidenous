import type { Rand } from '@/lib/utils/rand';

/** A straight piece of crack from (x, y), `len` px long at `deg`, drawn from `at` over `dur` ms. */
export type Segment = { x: number; y: number; len: number; deg: number; at: number; dur: number; thick: number };

/**
 * Broken glass: straight-ish radial cracks from (x, y), then a few concentric rings joining neighbouring cracks.
 * Each segment is drawn after the previous one on its line.
 */
export function cracks(r: Rand, x: number, y: number, at: number, w: number, h: number): Segment[] {
  const out: Segment[] = [];
  const line = (x0: number, y0: number, x1: number, y1: number, t0: number, thick: number) => {
    const len = Math.hypot(x1 - x0, y1 - y0);
    const dur = Math.max(18, len * 0.55);
    out.push({ x: x0, y: y0, len, deg: (Math.atan2(y1 - y0, x1 - x0) * 180) / Math.PI, at: t0, dur, thick });
    return t0 + dur;
  };
  const branches = 9;
  const rings = [70, 150];
  const pts: { x: number; y: number; t: number }[][] = [];
  for (let b = 0; b < branches; b++) {
    let ang = (b / branches) * Math.PI * 2 + r.range(-0.25, 0.25);
    let px = x + Math.cos(ang) * 10;
    let py = y + Math.sin(ang) * 10;
    let time = at + r.range(0, 40);
    let dist = 10;
    const marks: { x: number; y: number; t: number }[] = [];
    let ring = 0;
    for (let i = 0; i < 14 && px > -30 && px < w + 30 && py > -30 && py < h + 30; i++) {
      const len = r.range(36, 70);
      ang += r.range(-0.16, 0.16);
      const nx = px + Math.cos(ang) * len;
      const ny = py + Math.sin(ang) * len;
      time = line(px, py, nx, ny, time, Math.max(1.2, 3 - i * 0.2));
      dist += len;
      if (ring < rings.length && dist >= rings[ring] * r.range(0.85, 1.2)) {
        marks.push({ x: nx, y: ny, t: time });
        ring++;
      }
      px = nx;
      py = ny;
    }
    pts.push(marks);
  }
  for (let k = 0; k < rings.length; k++)
    for (let b = 0; b < branches; b++) {
      const p = pts[b][k];
      const q = pts[(b + 1) % branches][k];
      if (!p || !q || r.f() < 0.25) continue;
      const mx = (p.x + q.x) / 2 + r.range(-10, 10);
      const my = (p.y + q.y) / 2 + r.range(-10, 10);
      const t0 = Math.max(p.t, q.t) + 40;
      line(mx, my, p.x, p.y, line(mx, my, q.x, q.y, t0, 1.4) - 60, 1.4);
    }
  return out;
}
