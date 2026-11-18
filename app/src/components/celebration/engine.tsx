/**
 * The particle engine: one clock (ms since the party started, on the UI thread) drives everything.
 * Particles follow closed-form physics (gravity + linear drag), so each frame is a few maths per piece in a worklet:
 * no per-frame React render, no integration state.
 */
import { type ReactNode, useMemo } from 'react';
import { StyleSheet, Text, View, type ViewStyle } from 'react-native';
import Animated, { type SharedValue, useAnimatedStyle } from 'react-native-reanimated';

import { ballistic } from '@/lib/utils/ballistics';
import type { Rand } from '@/lib/utils/rand';

export { makeRand } from '@/lib/utils/rand';

export type Clock = SharedValue<number>;

// ——— Particles ———

export type Particle = {
  /** Start position (px, stage coordinates) and velocity (px/s). */
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** Gravity (px/s², positive = down) and linear drag (1/s). */
  g?: number;
  k?: number;
  /** Appears at `at` ms, gone `life` ms later. */
  at: number;
  life: number;
  w: number;
  h: number;
  color: string;
  shape: 'rect' | 'circle' | 'streak' | 'node' | 'text';
  rot?: number;
  /** deg/s */
  spin?: number;
  /** Paper flutter: the piece flips around its horizontal axis (rad/s). */
  flip?: number;
  /** Sideways sway (px) and its frequency (Hz): leaves, embers, bubbles. */
  sway?: number;
  swayHz?: number;
  fadeIn?: number;
  fadeOut?: number;
  /** Scale at birth and at death. */
  grow?: [number, number];
  /** shape 'text' */
  glyph?: string;
  font?: string;
  /** shape 'node': drawn in a w×h box (an Svg). */
  node?: ReactNode;
};

/** The numbers only: what the worklet captures (a ReactNode must never cross to the UI thread). */
function motionOf(p: Particle) {
  return {
    x: p.x,
    y: p.y,
    vx: p.vx,
    vy: p.vy,
    g: p.g ?? 0,
    k: Math.max(p.k ?? 0, 0.001),
    at: p.at,
    life: p.life,
    w: p.w,
    h: p.h,
    rot: p.rot ?? 0,
    spin: p.spin ?? 0,
    flip: p.flip ?? 0,
    sway: p.sway ?? 0,
    swayHz: p.swayHz ?? 1,
    fadeIn: p.fadeIn ?? 0,
    fadeOut: p.fadeOut ?? 0,
    g0: p.grow ? p.grow[0] : 1,
    g1: p.grow ? p.grow[1] : 1,
    streak: p.shape === 'streak' ? 1 : 0,
    speed0: Math.max(1, Math.hypot(p.vx, p.vy)),
  };
}

const HIDDEN = { opacity: 0, transform: [{ translateX: -9999 }, { translateY: 0 }, { rotate: '0deg' }, { scaleX: 1 }, { scaleY: 1 }] };

function Piece({ p, clock }: { p: Particle; clock: Clock }) {
  const m = useMemo(() => motionOf(p), [p]);
  const style = useAnimatedStyle(() => {
    const ms = clock.value - m.at;
    if (ms < 0 || ms > m.life) return HIDDEN;
    const t = ms / 1000;
    const at = ballistic(m.x, m.y, m.vx, m.vy, m.g, m.k, t);
    let x = at.x;
    const y = at.y;
    if (m.sway) x += m.sway * Math.sin(t * m.swayHz * 2 * Math.PI);
    let o = 1;
    if (m.fadeIn > 0 && ms < m.fadeIn) o = ms / m.fadeIn;
    if (m.fadeOut > 0 && ms > m.life - m.fadeOut) o = Math.min(o, (m.life - ms) / m.fadeOut);
    const s = m.g0 + (m.g1 - m.g0) * (ms / m.life);
    let rot = m.rot + m.spin * t;
    let sx = s;
    if (m.streak) {
      // Sparks point where they fly and shorten as they slow down.
      rot = (Math.atan2(at.vy, at.vx) * 180) / Math.PI;
      sx = s * Math.min(1, Math.max(0.25, Math.hypot(at.vx, at.vy) / m.speed0));
    }
    const flip = m.flip ? Math.cos(t * m.flip) : 1;
    return {
      opacity: o,
      transform: [{ translateX: x - m.w / 2 }, { translateY: y - m.h / 2 }, { rotate: `${rot}deg` }, { scaleX: sx }, { scaleY: s * flip }],
    };
  });
  const base: ViewStyle = { position: 'absolute', left: 0, top: 0, width: p.w, height: p.h };
  if (p.shape === 'node') return <Animated.View style={[base, style]}>{p.node}</Animated.View>;
  if (p.shape === 'text')
    return (
      <Animated.View style={[base, { alignItems: 'center', justifyContent: 'center' }, style]}>
        <Text style={{ color: p.color, fontSize: p.h * 0.85, lineHeight: p.h, fontFamily: p.font, includeFontPadding: false }}>{p.glyph}</Text>
      </Animated.View>
    );
  const radius = p.shape === 'rect' ? Math.min(2, p.w / 4) : p.h / 2;
  return <Animated.View style={[base, { backgroundColor: p.color, borderRadius: radius }, style]} />;
}

/** Draws a list of particles. The list must be stable (useMemo it). */
export function Particles({ clock, list }: { clock: Clock; list: Particle[] }) {
  return (
    <View style={StyleSheet.absoluteFill} pointerEvents="none">
      {list.map((p, i) => (
        <Piece key={i} p={p} clock={clock} />
      ))}
    </View>
  );
}

// ——— Emitters: plain functions returning particles ———

type Look = (r: Rand, i: number) => Partial<Particle>;

/** Paper confetti: rectangles and dots that flutter. */
export const paper =
  (colors: readonly string[]): Look =>
  (r, i) => {
    const dot = i % 5 === 0;
    const w = dot ? r.range(6, 9) : r.range(6, 10);
    return {
      shape: dot ? 'circle' : 'rect',
      color: r.pick(colors),
      w,
      h: dot ? w : r.range(10, 16),
      rot: r.range(0, 360),
      spin: r.sign() * r.range(120, 540),
      flip: dot ? 0 : r.range(6, 16),
      sway: r.range(4, 14),
      swayHz: r.range(0.6, 1.6),
    };
  };

/** Glowing sparks (fireworks, embers). */
export const sparks =
  (colors: readonly string[], size = 3): Look =>
  (r) => ({ shape: 'streak', color: r.pick(colors), w: r.range(size * 3, size * 6), h: size });

/** Radial burst from a point. Speeds in px/s. */
type Burst = {
  x: number;
  y: number;
  at: number;
  count: number;
  speed: [number, number];
  look: Look;
  life?: [number, number];
  g?: number;
  k?: number;
  /** Direction (deg, 0 = right, -90 = up) and spread (deg) for a cone instead of a full circle. */
  dir?: number;
  spread?: number;
  fadeOut?: number;
  grow?: [number, number];
  /** Evenly spaced angles (fireworks), else random. */
  ring?: boolean;
};

export function burst(r: Rand, o: Burst): Particle[] {
  const life = o.life ?? [900, 1500];
  return Array.from({ length: o.count }, (_, i) => {
    const a =
      o.spread != null
        ? (((o.dir ?? -90) + r.range(-o.spread / 2, o.spread / 2)) * Math.PI) / 180
        : o.ring
          ? (i / o.count) * Math.PI * 2 + r.range(-0.05, 0.05)
          : r.range(0, Math.PI * 2);
    const v = r.range(o.speed[0], o.speed[1]);
    return {
      x: o.x,
      y: o.y,
      vx: Math.cos(a) * v,
      vy: Math.sin(a) * v,
      g: o.g ?? 900,
      k: o.k ?? 1.6,
      at: o.at + r.range(0, 40),
      life: r.range(life[0], life[1]),
      fadeOut: o.fadeOut ?? 350,
      grow: o.grow,
      w: 8,
      h: 8,
      color: '#fff',
      shape: 'circle',
      ...o.look(r, i),
    } as Particle;
  });
}

/**
 * Two confetti cannons firing from the bottom corners of a `w` wide stage: the left one at `dir` (deg),
 * the right one mirrored, `delay` ms later.
 */
export function cannons(r: Rand, o: Omit<Burst, 'x' | 'dir'> & { w: number; dir: number; delay?: number }): Particle[] {
  const { w, dir, delay = 0, ...cone } = o;
  return [...burst(r, { ...cone, x: -10, dir }), ...burst(r, { ...cone, x: w + 10, at: cone.at + delay, dir: -180 - dir })];
}

/** Falling from above the top edge over a time window. */
export function rain(
  r: Rand,
  o: { w: number; h: number; count: number; from: number; to: number; look: Look; speed?: [number, number]; life?: number; k?: number },
): Particle[] {
  return Array.from({ length: o.count }, (_, i) => ({
    x: r.range(0, o.w),
    y: -20,
    vx: r.range(-40, 40),
    vy: r.range(...(o.speed ?? [120, 260])),
    g: 300,
    k: o.k ?? 1.1,
    at: r.range(o.from, o.to),
    life: o.life ?? 3200,
    fadeOut: 300,
    w: 8,
    h: 12,
    color: '#fff',
    shape: 'rect',
    ...o.look(r, i),
  })) as Particle[];
}

/**
 * A firework: a shell climbs from the bottom with a short trail, then bursts at (x, y) at `at + climb`.
 * Returns the particles and the burst time (for flashes, shake, haptics).
 */
export function firework(
  r: Rand,
  o: { x: number; y: number; from: number; at: number; colors: readonly string[]; count?: number; climb?: number; power?: number },
): { list: Particle[]; boom: number } {
  const climb = o.climb ?? 650;
  const T = climb / 1000;
  const h = o.from - o.y;
  const g = (2 * h) / (T * T);
  const shell: Particle = { x: o.x, y: o.from, vx: 0, vy: -g * T, g, k: 0, at: o.at, life: climb, w: 14, h: 3, color: '#FFF6D8', shape: 'streak' };
  // The trail: dim sparks left behind along the way, falling a little.
  const trail = Array.from({ length: 6 }, (_, i) => {
    const s = ((i + 1) / 7) * T;
    return {
      x: o.x + r.range(-2, 2),
      y: o.from - (g * T * s - (g * s * s) / 2),
      vx: r.range(-15, 15),
      vy: r.range(10, 50),
      g: 200,
      k: 2,
      at: o.at + s * 1000,
      life: 500,
      fadeOut: 450,
      w: 3,
      h: 3,
      color: '#FFE9A8',
      shape: 'circle',
    } as Particle;
  });
  const boom = o.at + climb;
  const power = o.power ?? 1;
  const main = burst(r, {
    x: o.x,
    y: o.y,
    at: boom,
    count: o.count ?? 34,
    speed: [380 * power, 520 * power],
    ring: true,
    g: 260,
    k: 2.2,
    life: [1100, 1500],
    fadeOut: 500,
    look: sparks(o.colors, 3),
  });
  const glitter = burst(r, {
    x: o.x,
    y: o.y,
    at: boom + 60,
    count: 10,
    speed: [60, 260 * power],
    g: 160,
    k: 2,
    life: [900, 1400],
    fadeOut: 600,
    look: (rr) => ({ shape: 'circle', color: rr.pick(o.colors), w: 3, h: 3 }),
  });
  return { list: [shell, ...trail, ...main, ...glitter], boom };
}
