/** A pie wedge of radius R centred on (R, R), pointing at angle `a` (rad), `half` radians wide on each side. */
export function wedgePath(R: number, a: number, half: number) {
  return `M${R},${R} L${R + Math.cos(a - half) * R},${R + Math.sin(a - half) * R} L${R + Math.cos(a + half) * R},${R + Math.sin(a + half) * R} Z`;
}

/** `count` evenly spaced light rays filling a `size` square. */
export function raysPath(size: number, count: number) {
  const R = size / 2;
  const half = Math.PI / count / 2.2;
  let d = '';
  for (let i = 0; i < count; i++) d += `${wedgePath(R, (i / count) * Math.PI * 2, half)} `;
  return d;
}

/** Annular sector between radii r0, r1 and angles a0, a1 (deg), centred on 0,0. */
export function sectorPath(r0: number, r1: number, a0: number, a1: number) {
  const p = (r: number, a: number) => `${(r * Math.cos((a * Math.PI) / 180)).toFixed(2)},${(r * Math.sin((a * Math.PI) / 180)).toFixed(2)}`;
  return `M${p(r0, a0)} L${p(r1, a0)} A${r1},${r1} 0 0 1 ${p(r1, a1)} L${p(r0, a1)} A${r0},${r0} 0 0 0 ${p(r0, a0)} Z`;
}
