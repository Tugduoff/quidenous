import type { Shape as AvatarShape } from '@/lib/utils/avatar';
import raw from '@/lib/utils/mascot-parts.json';

export type Shape = AvatarShape & { hd?: 1 };
export type Who = 'zeste' | 'bulle' | 'pave' | 'paillette' | 'roupi';
export type Mood = 'happy' | 'accuse' | 'sleepy' | 'celebrate' | 'sad' | 'think' | 'caught';
type Eyes = 'open' | 'lazy' | 'squint' | 'closed' | 'happy' | 'side' | 'wide' | 'teary' | 'stars';
type Brows = 'none' | 'up' | 'sus' | 'sad' | 'think' | 'worried';
type Mouth = 'smile' | 'grin' | 'smirk' | 'o' | 'gasp' | 'frown' | 'wail' | 'hmm' | 'wobble';
type Arm = 'rest' | 'wave' | 'up' | 'point' | 'aim' | 'droop' | 'finger' | 'hands' | 'none';
type Leg = 'stand' | 'wide' | 'shy' | 'tap' | 'tiptoe' | 'hop' | 'dangle' | 'sit' | 'none';
export type Prop = 'none' | 'zzz' | 'ask' | 'sparkles' | 'tear' | 'tears' | 'sweat';
export type Lod = 'hi' | 'lo';
export type Outfit = 'winter';

export type Pose = {
  eyes: Eyes;
  brows?: Brows;
  mouth: Mouth;
  arms: [Arm, Arm];
  legs: [Leg, Leg];
  prop?: Prop;
  tilt?: number;
  dx?: number;
  squash?: [number, number];
  lift?: number;
};

type Pt = [number, number];
type Limb = { z: 'back' | 'front'; lift?: number; els: Shape[]; joint: Shape[] };
type Body = {
  color: string;
  shade: string;
  cheek: string;
  acc2: string;
  d: string;
  gloss: string;
  face: [number, number, number];
  arms: [Pt, Pt];
  legs: [Pt, Pt];
  prop: Pt;
  shadow: number;
  back: Shape[];
  front: Shape[];
  faceUnder: Shape[];
  faceExtras: Shape[];
  wool: string;
  winter: { hat: Shape[] | null; top: Shape[]; scarf: Shape[] };
};
type Parts = {
  ink: string;
  ground: number;
  bodySw: number;
  cast: Who[];
  bodies: Record<Who, Body>;
  parts: {
    eyes: Record<Eyes, Shape[]>;
    brows: Record<Brows, Shape[]>;
    mouth: Record<Mouth, Shape[]>;
    arms: Record<Arm, Limb>;
    legs: Record<Leg, Limb>;
    props: Record<Prop, { at: 'prop' | 'face' | 'abs'; els: Shape[] }>;
  };
  moods: Record<Mood, Omit<Pose, 'arms' | 'legs'> & { arms: string[]; legs?: string[] }>;
  quirks: Partial<Record<Who, Partial<Record<Mood, Partial<Pose>>>>>;
};
export const MASCOT = raw as unknown as Parts;
export const CAST = MASCOT.cast;
const INK = MASCOT.ink;
export const GROUND = MASCOT.ground;
export const MOODS = Object.keys(MASCOT.moods) as Mood[];

export function resolvePose(who: Who, mood: Mood | Pose): Required<Pose> {
  const base = typeof mood === 'string' ? { ...MASCOT.moods[mood], ...MASCOT.quirks[who]?.[mood] } : mood;
  const p = { brows: 'none', prop: 'none', tilt: 0, dx: 0, squash: [1, 1], legs: ['stand', 'stand'], ...base } as Required<Pose>;
  if (base.lift === undefined) {
    const [a, b] = p.legs.map((k) => MASCOT.parts.legs[k].lift ?? 0);
    p.lift = Math.abs(a) >= Math.abs(b) ? a : b;
  }
  return p;
}

export type Group = { tr?: string; shapes: Shape[] };
export type Composed = {
  body: string;
  back: Group[];
  fill: { d: string; color: string; shade: string; gloss: string | null; outline: string; sw: number };
  over: Group[];
  under: Group;
  eyes: Group;
  blink: Group | null;
  face: Group;
  front: Group[];
  scarf: Group | null;
  hat: Group | null;
  hatTop: Group | null;
  prop: (Group & { kind: Prop; at: 'prop' | 'face' | 'abs'; pivot: [number, number] }) | null;
  shadow: { cx: number; rx: number; ry: number; air: boolean };
  lift: number;
  point: -1 | 0 | 1;
  nodes: number;
};

const POINTS: Partial<Record<Arm, true>> = { point: true, aim: true };
const BLINKS: Partial<Record<Eyes, true>> = { open: true, teary: true, side: true, lazy: true, squint: true };

function paint(shapes: Shape[], pal: Record<string, string>, lod: Lod): Shape[] {
  const out: Shape[] = [];
  for (const s of shapes) {
    if (lod === 'lo' && s.hd) continue;
    out.push({ ...s, fill: s.fill && (pal[s.fill] ?? s.fill), stroke: s.stroke && (pal[s.stroke] ?? s.stroke) });
  }
  return out;
}

export function darken(hex: string, k = 0.18) {
  const n = parseInt(hex.slice(1), 16);
  const c = [n >> 16, (n >> 8) & 255, n & 255].map((v) => Math.round(v * (1 - k)));
  return `#${c.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

const cache = new Map<string, Composed>();
const poseCache = new WeakMap<Pose, Map<string, Composed>>();

export function compose(who: Who, mood: Mood | Pose = 'happy', opts: { lod?: Lod; prop?: Prop; tint?: string; outfit?: Outfit } = {}): Composed {
  const lod = opts.lod ?? 'hi';
  const sub = `${lod}|${opts.prop ?? ''}|${opts.tint ?? ''}|${opts.outfit ?? ''}`;
  const key = typeof mood === 'string' ? `${who}|${mood}|${sub}` : null;
  const hit = key ? cache.get(key) : typeof mood === 'object' ? poseCache.get(mood)?.get(sub) : undefined;
  if (hit) return hit;

  const b = MASCOT.bodies[who];
  const P = MASCOT.parts;
  const m = resolvePose(who, mood);
  const color = opts.tint ?? b.color;
  const shade = opts.tint ? darken(opts.tint) : b.shade;
  const pal: Record<string, string> = { ink: INK, body: color, shade, cheek: b.cheek, white: '#FFFFFF', acc2: b.acc2, wool: b.wool, wool2: darken(b.wool) };
  const draw = (shapes: Shape[]) => paint(shapes, pal, lod);

  const limbs = (group: 'arms' | 'legs', names: [Arm, Arm] | [Leg, Leg]) =>
    names.map((name, i) => {
      const part = (P[group] as Record<string, Limb>)[name];
      const [x, y] = b[group][i];
      const tr = `translate(${x} ${y})${i === 0 ? ' scale(-1 1)' : ''}`;
      return { z: part.z, el: { tr, shapes: draw(part.els) }, joint: { tr, shapes: draw(part.joint) } };
    });
  const arms = limbs('arms', m.arms);
  const legs = limbs('legs', m.legs);
  const all = [...legs, ...arms];
  const layer = (z: 'back' | 'front') => [...legs, ...arms].filter((l) => l.z === z && l.el.shapes.length).map((l) => l.el);

  const [fx, fy, fs] = b.face;
  const faceTr = `translate(${fx} ${fy})${fs !== 1 ? ` scale(${fs})` : ''}`;
  const cheeks: Shape[] = [
    { el: 'ellipse', cx: -34, cy: 14, rx: 8.5, ry: 5, fill: 'cheek', op: 0.85 },
    { el: 'ellipse', cx: 34, cy: 14, rx: 8.5, ry: 5, fill: 'cheek', op: 0.85 },
  ];
  const under: Group = { tr: faceTr, shapes: draw([...cheeks, ...b.faceUnder]) };
  const face: Group = { tr: faceTr, shapes: draw([...b.faceExtras, ...(lod === 'lo' ? [] : P.brows[m.brows]), ...P.mouth[m.mouth]]) };
  const eyes: Group = { tr: faceTr, shapes: draw(P.eyes[m.eyes]) };
  const blink = BLINKS[m.eyes] ? { tr: faceTr, shapes: draw(P.eyes.closed) } : null;

  const propKind: Prop = opts.prop ?? m.prop;
  const pp = P.props[propKind];
  const prop =
    propKind === 'none' || !pp.els.length
      ? null
      : {
          kind: propKind,
          at: pp.at,
          tr: pp.at === 'face' ? faceTr : pp.at === 'prop' ? `translate(${b.prop[0]} ${b.prop[1]})` : undefined,
          shapes: draw(pp.els),
          pivot: (pp.at === 'prop' ? [b.prop[0] + m.dx + (propKind === 'ask' ? 16 : 18), b.prop[1] + m.lift + (propKind === 'ask' ? -6 : 0)] : [100, 100]) as [number, number],
        };

  const [sx, sy] = m.squash;
  const body =
    `translate(${m.dx} ${m.lift}) ` +
    (m.tilt || sx !== 1 || sy !== 1 ? `translate(100 ${GROUND}) rotate(${m.tilt}) scale(${sx} ${sy}) translate(-100 -${GROUND})` : '');
  const air = m.lift < 0;
  const winter = opts.outfit === 'winter' ? b.winter : null;
  const hat = winter?.hat ? { shapes: draw(winter.hat) } : null;
  const hatTop = winter?.top.length ? { shapes: draw(winter.top) } : null;
  const scarfFitsAtThisSize = lod === 'hi';
  const c: Composed = {
    body: body.trim(),
    back: [{ shapes: hat ? [] : draw(b.back) }, ...layer('back')].filter((g) => g.shapes.length),
    fill: { d: b.d, color, shade, gloss: lod === 'lo' ? null : b.gloss, outline: INK, sw: MASCOT.bodySw },
    over: [...(lod === 'lo' ? [] : all.map((l) => l.joint).filter((g) => g.shapes.length)), { shapes: draw(b.front) }].filter((g) => g.shapes.length),
    under,
    eyes,
    blink,
    face,
    front: layer('front'),
    scarf: winter && scarfFitsAtThisSize ? { shapes: draw(winter.scarf) } : null,
    hat,
    hatTop,
    prop,
    shadow: { cx: 100 + m.dx, rx: b.shadow * sx * (air ? 0.7 : 1), ry: air ? 5.5 : 7, air },
    lift: m.lift,
    point: POINTS[m.arms[1]] ? 1 : POINTS[m.arms[0]] ? -1 : 0,
    nodes: 0,
  };
  c.nodes = countNodes(c, lod);
  if (key) cache.set(key, c);
  else if (typeof mood === 'object') {
    const m2 = poseCache.get(mood) ?? new Map<string, Composed>();
    m2.set(sub, c);
    poseCache.set(mood, m2);
  }
  return c;
}

function countNodes(c: Composed, lod: Lod) {
  const n = (gs: (Group | null)[]) => gs.reduce((s, g) => s + (g ? g.shapes.length : 0), 0);
  const fill = lod === 'lo' ? 2 : 4 + (c.fill.gloss ? 1 : 0);
  return n(c.back) + fill + n(c.over) + n([c.scarf, c.under, c.eyes, c.blink, c.face, c.hat, c.hatTop]) + n(c.front) + n([c.prop]);
}
