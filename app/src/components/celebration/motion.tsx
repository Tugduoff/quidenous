/** Animated wrappers driven by the scene clock: reveals, slams, pops, keyframes and simple trajectories. */
import type { ReactNode } from 'react';
import type { ViewStyle } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';

import { ballistic } from '@/lib/utils/ballistics';
import { clamp01, easeOutCubic } from '@/lib/utils/easing';

import type { Clock } from '@/components/celebration/engine';
import { ABS } from '@/components/celebration/layout';

/** Fades and rises in at `at`. */
export function Reveal({
  clock,
  at,
  children,
  rise = 14,
  dur = 380,
  style,
}: {
  clock: Clock;
  at: number;
  children: ReactNode;
  rise?: number;
  dur?: number;
  style?: ViewStyle;
}) {
  const a = useAnimatedStyle(() => {
    const p = clamp01((clock.value - at) / dur);
    const e = 1 - Math.pow(1 - p, 3);
    return { opacity: e, transform: [{ translateY: (1 - e) * rise }] };
  });
  return <Animated.View style={[style, a]}>{children}</Animated.View>;
}

/**
 * Typography slam: falls from big onto the screen at `at` (the impact), squashes, stretches back and settles.
 * Lands from `from`× its size over `fall` ms.
 */
export function Slam({
  clock,
  at,
  children,
  from = 3.2,
  fall = 200,
  style,
}: {
  clock: Clock;
  at: number;
  children: ReactNode;
  from?: number;
  fall?: number;
  style?: ViewStyle;
}) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const T = [at - fall, at, at + 70, at + 170, at + 290, at + 420];
    const sx = interpolate(c, T, [from, 1, 1.26, 0.92, 1.03, 1], Extrapolation.CLAMP);
    const sy = interpolate(c, T, [from, 1, 0.74, 1.1, 0.98, 1], Extrapolation.CLAMP);
    const o = interpolate(c, [at - fall, at - fall * 0.4], [0, 1], Extrapolation.CLAMP);
    return { opacity: o, transform: [{ scaleX: sx }, { scaleY: sy }] };
  });
  return <Animated.View style={[style, a]}>{children}</Animated.View>;
}

/** Pops up from small with one bouncy overshoot. */
export function Pop({ clock, at, children, style }: { clock: Clock; at: number; children: ReactNode; style?: ViewStyle }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const s = interpolate(c, [at, at + 180, at + 300, at + 420], [0.2, 1.12, 0.97, 1], Extrapolation.CLAMP);
    return { opacity: interpolate(c, [at, at + 90], [0, 1], Extrapolation.CLAMP), transform: [{ scale: s }] };
  });
  return <Animated.View style={[style, a]}>{children}</Animated.View>;
}

type Keys = { t: number[]; x?: number[]; y?: number[]; r?: number[]; s?: number[]; sx?: number[]; sy?: number[]; o?: number[] };

/** Keyframes on the clock: translate, rotate, scale, opacity (each optional, all on the same times `k.t`, clamped). */
export function Keyed({ clock, k, style, children }: { clock: Clock; k: Keys; style?: ViewStyle; children?: ReactNode }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const T = k.t;
    const f = (v: number[] | undefined, d: number) => (v ? interpolate(c, T, v, Extrapolation.CLAMP) : d);
    const s = f(k.s, 1);
    return {
      opacity: f(k.o, 1),
      transform: [
        { translateX: f(k.x, 0) },
        { translateY: f(k.y, 0) },
        { rotate: `${f(k.r, 0)}deg` },
        { scaleX: s * f(k.sx, 1) },
        { scaleY: s * f(k.sy, 1) },
      ],
    };
  });
  return (
    <Animated.View pointerEvents="none" style={[style, a]}>
      {children}
    </Animated.View>
  );
}

/** Sits still at (x, y) until `at`, then flies off with gravity, drag and spin (bowling pins, torn pages). */
export type Throw = { x: number; y: number; vx: number; vy: number; g: number; k: number; spin: number; rot: number; at: number; w: number; h: number };
export function Thrown({ clock, p, children }: { clock: Clock; p: Throw; children: ReactNode }) {
  const a = useAnimatedStyle(() => {
    const t = Math.max(0, clock.value - p.at) / 1000;
    const { x, y } = ballistic(p.x, p.y, p.vx, p.vy, p.g, Math.max(p.k, 0.001), t);
    return { transform: [{ translateX: x - p.w / 2 }, { translateY: y - p.h / 2 }, { rotate: `${p.rot + p.spin * t}deg` }] };
  });
  return <Animated.View style={[{ position: 'absolute', left: 0, top: 0, width: p.w, height: p.h }, a]}>{children}</Animated.View>;
}

/**
 * Something orbiting (cx, cy): its angle turns at `spin` rad/s, its radius eases from r0 to r1 after `at`.
 * The orbit is an ellipse (`tilt` squashes it) turned by `tiltRot` radians.
 */
type Orbit = { r0: number; r1: number; a: number; spin: number; at: number; dur: number; size: number };
export function Orbiter({
  clock,
  o,
  cx,
  cy,
  tilt = 1,
  tiltRot = 0,
  children,
  style,
}: {
  clock: Clock;
  o: Orbit;
  cx: number;
  cy: number;
  tilt?: number;
  tiltRot?: number;
  children?: ReactNode;
  style?: ViewStyle;
}) {
  const { r0, r1, a, spin, at, dur, size } = o;
  const cosR = Math.cos(tiltRot);
  const sinR = Math.sin(tiltRot);
  const anim = useAnimatedStyle(() => {
    const ms = clock.value;
    if (ms < at) return { opacity: 0, transform: [{ translateX: -9999 }, { translateY: 0 }] };
    const b = easeOutCubic(clamp01((ms - at) / dur));
    const r = r0 + (r1 - r0) * b;
    const ang = a + (spin * ms) / 1000;
    const ex = r * Math.cos(ang);
    const ey = r * Math.sin(ang) * tilt;
    const x = cx + ex * cosR - ey * sinR;
    const y = cy + ex * sinR + ey * cosR;
    return { opacity: clamp01(((ms - at) / dur) * 2), transform: [{ translateX: x - size / 2 }, { translateY: y - size / 2 }] };
  });
  return (
    <Animated.View style={[ABS, { width: size, height: size, alignItems: 'center', justifyContent: 'center' }, style, anim]}>
      {children}
    </Animated.View>
  );
}

/** Visible only between `from` and `to` (ms). */
export function Window({ clock, from, to, children, style }: { clock: Clock; from: number; to: number; children: ReactNode; style?: ViewStyle }) {
  const anim = useAnimatedStyle(() => ({ opacity: clock.value >= from && clock.value < to ? 1 : 0 }));
  return <Animated.View style={[style, anim]}>{children}</Animated.View>;
}

/** Fades from 1 to 0 between `from` and `to`. */
export function FadeAway({ clock, from, to, children, style }: { clock: Clock; from: number; to: number; children: ReactNode; style?: ViewStyle }) {
  const anim = useAnimatedStyle(() => ({ opacity: interpolate(clock.value, [from, to], [1, 0], Extrapolation.CLAMP) }));
  return <Animated.View style={[style, anim]}>{children}</Animated.View>;
}
