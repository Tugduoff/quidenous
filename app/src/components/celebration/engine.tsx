/**
 * The celebration engine: one clock (ms since the party started, on the UI thread) drives everything.
 * Particles follow closed-form physics (gravity + linear drag), so each frame is a few maths per piece in a worklet:
 * no per-frame React render, no integration state. Effects (shockwaves, rays, flashes, shake) read the same clock.
 */
import { type ReactNode, useEffect, useMemo } from 'react';
import { StyleSheet, Text, type TextStyle, View, type ViewStyle } from 'react-native';
import Animated, {
  cancelAnimation,
  Easing,
  Extrapolation,
  interpolate,
  type SharedValue,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from 'react-native-reanimated';
import Svg, { Circle, Defs, Path, RadialGradient, Stop } from 'react-native-svg';

export type Clock = SharedValue<number>;

/** Milliseconds since the party started. `frozen` pins it to one instant (dev captures). */
export function useClock(duration: number, frozen?: number | null) {
  const c = useSharedValue(frozen ?? 0);
  useEffect(() => {
    if (frozen != null) return;
    c.value = withTiming(duration, { duration, easing: Easing.linear });
    return () => cancelAnimation(c);
  }, [c, duration, frozen]);
  return c;
}

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
    const decay = Math.exp(-m.k * t);
    const e = 1 - decay;
    const term = m.g / m.k;
    let x = m.x + (m.vx / m.k) * e;
    const y = m.y + term * t + ((m.vy - term) / m.k) * e;
    if (m.sway) x += m.sway * Math.sin(t * m.swayHz * 2 * Math.PI);
    let o = 1;
    if (m.fadeIn > 0 && ms < m.fadeIn) o = ms / m.fadeIn;
    if (m.fadeOut > 0 && ms > m.life - m.fadeOut) o = Math.min(o, (m.life - ms) / m.fadeOut);
    const s = m.g0 + (m.g1 - m.g0) * (ms / m.life);
    let rot = m.rot + m.spin * t;
    let sx = s;
    if (m.streak) {
      // Sparks point where they fly and shorten as they slow down.
      const vx = m.vx * decay;
      const vy = term + (m.vy - term) * decay;
      rot = (Math.atan2(vy, vx) * 180) / Math.PI;
      sx = s * Math.min(1, Math.max(0.25, Math.hypot(vx, vy) / m.speed0));
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
export function burst(
  r: Rand,
  o: {
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
  },
): Particle[] {
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

// ——— Effects ———

const clamp01 = (v: number) => {
  'worklet';
  return Math.min(1, Math.max(0, v));
};

/** A ring that expands and fades from (x, y). */
export function Shockwave({
  clock,
  at,
  x,
  y,
  color,
  size = 360,
  dur = 650,
  width = 6,
}: {
  clock: Clock;
  at: number;
  x: number;
  y: number;
  color: string;
  size?: number;
  dur?: number;
  width?: number;
}) {
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - at) / dur);
    const on = clock.value >= at && p < 1;
    const ease = 1 - Math.pow(1 - p, 3);
    return { opacity: on ? (1 - p) * 0.9 : 0, transform: [{ scale: 0.05 + ease * 0.95 }] };
  });
  return (
    <Animated.View
      pointerEvents="none"
      style={[
        {
          position: 'absolute',
          left: x - size / 2,
          top: y - size / 2,
          width: size,
          height: size,
          borderRadius: size / 2,
          borderWidth: width,
          borderColor: color,
        },
        style,
      ]}
    />
  );
}

/** A full-screen colour flash peaking at `at`. */
export function Flash({
  clock,
  at,
  color,
  peak = 0.6,
  rise = 40,
  fall = 420,
}: {
  clock: Clock;
  at: number;
  color: string;
  peak?: number;
  rise?: number;
  fall?: number;
}) {
  const style = useAnimatedStyle(() => ({
    opacity: interpolate(clock.value, [at - rise, at, at + fall], [0, peak, 0], Extrapolation.CLAMP),
  }));
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: color }, style]} />;
}

/** Light rays turning slowly behind the number. */
export function Rays({
  clock,
  at,
  x,
  y,
  color,
  size = 620,
  count = 14,
  opacity = 0.22,
  speed = 18,
}: {
  clock: Clock;
  at: number;
  x: number;
  y: number;
  color: string;
  size?: number;
  count?: number;
  opacity?: number;
  speed?: number;
}) {
  const path = useMemo(() => {
    const R = size / 2;
    const half = Math.PI / count / 2.2;
    let d = '';
    for (let i = 0; i < count; i++) {
      const a = (i / count) * Math.PI * 2;
      d += `M${R},${R} L${R + Math.cos(a - half) * R},${R + Math.sin(a - half) * R} L${R + Math.cos(a + half) * R},${R + Math.sin(a + half) * R} Z `;
    }
    return d;
  }, [size, count]);
  const style = useAnimatedStyle(() => {
    const ms = clock.value - at;
    const p = clamp01(ms / 500);
    return {
      opacity: p * opacity,
      transform: [{ rotate: `${(Math.max(0, ms) / 1000) * speed}deg` }, { scale: 0.6 + 0.4 * (1 - Math.pow(1 - p, 3)) }],
    };
  });
  const id = `ray${color.replace('#', '')}`;
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: x - size / 2, top: y - size / 2, width: size, height: size }, style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={color} stopOpacity={1} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Path d={path} fill={`url(#${id})`} />
      </Svg>
    </Animated.View>
  );
}

/** A soft round glow (radial gradient), fading in at `at` and breathing a little. */
export function Glow({
  clock,
  at,
  x,
  y,
  color,
  size = 320,
  opacity = 0.5,
  pulse = 0,
}: {
  clock: Clock;
  at: number;
  x: number;
  y: number;
  color: string;
  size?: number;
  opacity?: number;
  pulse?: number;
}) {
  const style = useAnimatedStyle(() => {
    const ms = clock.value - at;
    const p = clamp01(ms / 400);
    const beat = pulse ? 1 + 0.08 * Math.sin((Math.max(0, ms) / 1000) * pulse * Math.PI * 2) : 1;
    return { opacity: p * opacity, transform: [{ scale: (0.7 + 0.3 * p) * beat }] };
  });
  const id = `glow${color.replace('#', '')}`;
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: x - size / 2, top: y - size / 2, width: size, height: size }, style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={color} stopOpacity={1} />
            <Stop offset="0.45" stopColor={color} stopOpacity={0.35} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={size / 2} cy={size / 2} r={size / 2} fill={`url(#${id})`} />
      </Svg>
    </Animated.View>
  );
}

/** Impacts: when they hit (ms) and how hard (px). Shake decays fast; strong ones also buzz the phone. */
export type Impact = { at: number; amp: number };

/** Style for the whole stage: a short, decaying shake after each impact. */
export function useShake(clock: Clock, impacts: Impact[]) {
  const list = useMemo(() => impacts.map((i) => [i.at, i.amp]), [impacts]);
  return useAnimatedStyle(() => {
    let x = 0;
    let y = 0;
    for (let i = 0; i < list.length; i++) {
      const dt = (clock.value - list[i][0]) / 1000;
      if (dt < 0 || dt > 0.45) continue;
      const a = list[i][1] * Math.exp(-dt * 11);
      x += a * Math.sin(dt * 95);
      y += a * 0.7 * Math.cos(dt * 120);
    }
    return { transform: [{ translateX: x }, { translateY: y }] };
  });
}

// ——— Typography ———

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

/** Ease-out with a small overshoot. */
function backOut(x: number) {
  'worklet';
  const c1 = 1.4;
  const c3 = c1 + 1;
  return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2);
}

function RollDigit({
  clock,
  digit,
  start,
  dur,
  size,
  color,
  font,
  spins,
}: {
  clock: Clock;
  digit: number;
  start: number;
  dur: number;
  size: number;
  color: string;
  font: string;
  spins: number;
}) {
  const lineH = Math.round(size * 1.02);
  // A strip of digits counting up into the target: 0…9 `spins` times, then up to the digit.
  const strip = useMemo(() => {
    const out: number[] = [];
    for (let i = 0; i < spins * 10 + digit + 1; i++) out.push(i % 10);
    return out;
  }, [digit, spins]);
  const end = (strip.length - 1) * lineH;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - start) / dur);
    return { transform: [{ translateY: -backOut(p) * end }] };
  });
  const text: TextStyle = {
    fontFamily: font,
    fontSize: size,
    lineHeight: lineH,
    height: lineH,
    color,
    textAlign: 'center',
    letterSpacing: -2,
    includeFontPadding: false,
  };
  return (
    <View style={{ height: lineH, overflow: 'hidden' }}>
      <Animated.View style={style}>
        {strip.map((n, i) => (
          <Text key={i} style={text}>
            {n}
          </Text>
        ))}
      </Animated.View>
    </View>
  );
}

/** A counter roll: each digit spins up into place, left to right. Ends around `at + dur + (digits-1) × stagger`. */
export function RollNumber({
  clock,
  value,
  at,
  size,
  color,
  font,
  dur = 900,
  stagger = 140,
}: {
  clock: Clock;
  value: number;
  at: number;
  size: number;
  color: string;
  font: string;
  dur?: number;
  stagger?: number;
}) {
  const digits = String(value).split('').map(Number);
  return (
    <View style={{ flexDirection: 'row' }}>
      {digits.map((d, i) => (
        <RollDigit
          key={i}
          clock={clock}
          digit={d}
          start={at + i * stagger}
          dur={dur}
          size={size}
          color={color}
          font={font}
          spins={1 + (digits.length - i > 1 ? 1 : 0)}
        />
      ))}
    </View>
  );
}

/** Big number text with an optional glow (text shadow). */
export function BigNumber({
  value,
  size,
  color,
  font,
  glow,
  style,
}: {
  value: string | number;
  size: number;
  color: string;
  font: string;
  glow?: string;
  style?: TextStyle;
}) {
  return (
    <Text
      style={[
        {
          fontFamily: font,
          fontSize: size,
          lineHeight: Math.round(size * 1.04),
          color,
          letterSpacing: -3,
          textAlign: 'center',
          includeFontPadding: false,
        },
        glow ? { textShadowColor: glow, textShadowRadius: 24, textShadowOffset: { width: 0, height: 0 } } : null,
        style,
      ]}>
      {value}
    </Text>
  );
}
