/** Medium parties with a theme of their own: fun numbers that deserve more than the generic fireworks. */
import { type ReactNode, useMemo } from 'react';
import { StyleSheet, Text, View, type ViewStyle } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, Ellipse, G, LinearGradient, Path, RadialGradient, Rect, Stop } from 'react-native-svg';

import { useT } from '@/lib/i18n';
import type { Theme } from '@/lib/theme';

import { BigNumber, burst, type Clock, Flash, Glow, paper, type Particle, Particles, Pop, Rays, Shockwave, Slam, sparks } from '../engine';
import { Headline, numberSize, party, type SceneFactory, type SceneProps } from '../scene';

// ——— Local helpers ———

const isDark = (t: Theme) => parseInt(t.bg.slice(1, 3), 16) < 128;

/** Cheap deterministic noise in [0, 1), usable in worklets. */
const hash = (x: number) => {
  'worklet';
  const s = Math.sin(x * 12.9898 + 78.233) * 43758.5453;
  return s - Math.floor(s);
};

type Keys = { t: number[]; x?: number[]; y?: number[]; r?: number[]; s?: number[]; sx?: number[]; sy?: number[]; o?: number[] };

/** Keyframes on the clock: translate, rotate, scale, opacity (each optional, all on the same times). */
function Keyed({ clock, k, style, children }: { clock: Clock; k: Keys; style?: ViewStyle; children?: ReactNode }) {
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
type Throw = { x: number; y: number; vx: number; vy: number; g: number; k: number; spin: number; rot: number; at: number; w: number; h: number };
function Thrown({ clock, p, children }: { clock: Clock; p: Throw; children: ReactNode }) {
  const a = useAnimatedStyle(() => {
    const t = Math.max(0, clock.value - p.at) / 1000;
    const k = Math.max(p.k, 0.001);
    const e = 1 - Math.exp(-k * t);
    const term = p.g / k;
    const x = p.x + (p.vx / k) * e;
    const y = p.y + term * t + ((p.vy - term) / k) * e;
    return { transform: [{ translateX: x - p.w / 2 }, { translateY: y - p.h / 2 }, { rotate: `${p.rot + p.spin * t}deg` }] };
  });
  return <Animated.View style={[{ position: 'absolute', left: 0, top: 0, width: p.w, height: p.h }, a]}>{children}</Animated.View>;
}

/** Positioned box centred on (x, y). */
const at = (x: number, y: number, w: number, h: number): ViewStyle => ({
  position: 'absolute',
  left: x - w / 2,
  top: y - h / 2,
  width: w,
  height: h,
});

// ——— 101: dalmatians ———

function Spot({ size, rot, a, b }: { size: number; rot: number; a: number; b: number }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 100 100">
      <G transform={`rotate(${rot} 50 50)`}>
        <Ellipse cx={50} cy={52} rx={34} ry={27} fill="#141414" />
        <Ellipse cx={50 + a * 70} cy={50 + b * 50} rx={19} ry={16} fill="#141414" />
        <Ellipse cx={50 - b * 50} cy={52 + a * 40} rx={14} ry={12} fill="#141414" />
      </G>
    </Svg>
  );
}

function Paw({ size, color, rot }: { size: number; color: string; rot: number }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 40 40">
      <G transform={`rotate(${rot} 20 20)`} fill={color}>
        <Ellipse cx={20} cy={26} rx={9.5} ry={8} />
        <Ellipse cx={8.5} cy={15} rx={3.8} ry={4.8} />
        <Ellipse cx={15.5} cy={8.5} rx={3.8} ry={4.8} />
        <Ellipse cx={24.5} cy={8.5} rx={3.8} ry={4.8} />
        <Ellipse cx={31.5} cy={15} rx={3.8} ry={4.8} />
      </G>
    </Svg>
  );
}

const dalmatians: SceneFactory = (m, t) => {
  const slam = 1500;
  return {
    duration: 4000,
    bg: '#F7F4EE',
    dim: 0.98,
    impacts: [{ at: slam, amp: 7 }],
    Render: function Dalmatians({ clock, w, h, rand, cx, cy }: SceneProps) {
      const spots = useMemo(() => {
        const out: { x: number; y: number; s: number; rot: number; at: number; a: number; b: number }[] = [];
        for (let tries = 0; out.length < 22 && tries < 400; tries++) {
          const x = rand.range(0, w);
          const y = rand.range(10, h - 10);
          // Keep the number and its text readable.
          if (Math.abs(y - (cy + 60)) < 175 && Math.abs(x - cx) < 175) continue;
          if (out.some((o) => Math.hypot(o.x - x, o.y - y) < 70)) continue;
          out.push({
            x,
            y,
            s: rand.range(34, 74),
            rot: rand.range(0, 180),
            at: rand.range(0, 1300),
            a: rand.range(-0.5, 0.5),
            b: rand.range(-0.5, 0.5),
          });
        }
        return out;
      }, [rand, w, h, cx, cy]);
      const paws = useMemo(() => {
        const lane = (y0: number, y1: number, from: number, dir: 1 | -1, n: number) =>
          Array.from({ length: n }, (_, i) => {
            const k = i / (n - 1);
            const x = dir === 1 ? -10 + k * (w + 20) : w + 10 - k * (w + 20);
            return { x, y: y0 + (y1 - y0) * k + (i % 2 ? 11 : -11), at: from + i * 170, rot: dir === 1 ? 90 : -90 };
          });
        return [...lane(h * 0.86, h * 0.8, 300, 1, 9), ...lane(h * 0.1, h * 0.14, 1700, -1, 9)];
      }, [w, h]);
      const pop = useMemo(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 22,
            speed: [300, 750],
            g: 900,
            k: 2,
            life: [900, 1400],
            look: (r) => {
              const s = r.range(6, 13);
              return { shape: 'circle', color: '#141414', w: s, h: s };
            },
          }),
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          {spots.map((s, i) => (
            <Keyed key={i} clock={clock} style={at(s.x, s.y, s.s, s.s)} k={{ t: [s.at, s.at + 120, s.at + 220], s: [0, 1.15, 1], o: [0, 1, 1] }}>
              <Spot size={s.s} rot={s.rot} a={s.a} b={s.b} />
            </Keyed>
          ))}
          {paws.map((p, i) => (
            <Keyed
              key={`p${i}`}
              clock={clock}
              style={at(p.x, p.y, 26, 26)}
              k={{ t: [p.at, p.at + 90, p.at + 1100, p.at + 1500], s: [0.4, 1, 1, 1], o: [0, 0.85, 0.85, 0] }}>
              <Paw size={26} color="#3A3A3A" rot={p.rot} />
            </Keyed>
          ))}
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#141414" size={420} width={5} />
          <Particles clock={clock} list={pop} />
          <Headline clock={clock} m={m} t={t} at={slam + 250} cy={cy} box={size * 1.1} ink="#121216" sub="#55555E">
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color="#121216" font={t.font.display} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

// ——— 112: siren ———

const RED = '#FF1E3C';
const BLUE = '#1E5BFF';

/** Two light cones (red, blue) turning around the beacon. */
function Beams({ clock, x, y, size }: { clock: Clock; x: number; y: number; size: number }) {
  const path = (a: number) => {
    const R = size / 2;
    const half = (20 * Math.PI) / 180;
    return `M${R},${R} L${R + Math.cos(a - half) * R},${R + Math.sin(a - half) * R} L${R + Math.cos(a + half) * R},${R + Math.sin(a + half) * R} Z`;
  };
  const style = useAnimatedStyle(() => ({
    opacity: interpolate(clock.value, [100, 400], [0, 1], Extrapolation.CLAMP),
    transform: [{ rotate: `${(clock.value / 1300) * 360}deg` }],
  }));
  return (
    <Animated.View pointerEvents="none" style={[at(x, y, size, size), style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id="beamR" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={RED} stopOpacity={0.75} />
            <Stop offset="1" stopColor={RED} stopOpacity={0} />
          </RadialGradient>
          <RadialGradient id="beamB" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={BLUE} stopOpacity={0.75} />
            <Stop offset="1" stopColor={BLUE} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Path d={path(0)} fill="url(#beamR)" />
        <Path d={path(Math.PI)} fill="url(#beamB)" />
      </Svg>
    </Animated.View>
  );
}

/** Whole-screen wash alternating red and blue; `phase` 0 = red, 1 = blue. */
function Wash({ clock, color, phase, peak, period = 520 }: { clock: Clock; color: string; phase: 0 | 1; peak: number; period?: number }) {
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    if (c < 150) return { opacity: 0 };
    const s = Math.sin(((c - 150) / period) * Math.PI * 2);
    const v = phase === 0 ? Math.max(0, s) : Math.max(0, -s);
    return { opacity: v * v * v * peak };
  });
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: color }, style]} />;
}

/** The light bar on top of the screen: two domes whose halos blink in turn. */
function LightBar({ clock, x, y }: { clock: Clock; x: number; y: number }) {
  return (
    <>
      <Halo clock={clock} x={x - 30} y={y} color={RED} phase={0} />
      <Halo clock={clock} x={x + 30} y={y} color={BLUE} phase={1} />
      <View pointerEvents="none" style={at(x, y + 6, 120, 50)}>
        <Svg width={120} height={50} viewBox="0 0 120 50">
          <Path d="M8,30 C8,12 22,6 42,6 L58,6 L58,30 Z" fill={RED} />
          <Path d="M62,6 L78,6 C98,6 112,12 112,30 L62,30 Z" fill={BLUE} />
          <Path d="M22,12 C26,9 32,8 40,8" stroke="#FFFFFF" strokeOpacity={0.7} strokeWidth={3} strokeLinecap="round" fill="none" />
          <Path d="M80,8 C88,8 94,9 98,12" stroke="#FFFFFF" strokeOpacity={0.7} strokeWidth={3} strokeLinecap="round" fill="none" />
          <Rect x={2} y={30} width={116} height={14} rx={5} fill="#2A2D3A" />
        </Svg>
      </View>
    </>
  );
}

function Halo({ clock, x, y, color, phase }: { clock: Clock; x: number; y: number; color: string; phase: 0 | 1 }) {
  const size = 260;
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const s = Math.sin(((c - 150) / 520) * Math.PI * 2);
    const v = c < 150 ? 0 : phase === 0 ? Math.max(0, s) : Math.max(0, -s);
    return { opacity: 0.25 + v * 0.75, transform: [{ scale: 0.8 + v * 0.3 }] };
  });
  const id = `halo${phase}`;
  return (
    <Animated.View pointerEvents="none" style={[at(x, y, size, size), style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={color} stopOpacity={0.9} />
            <Stop offset="0.35" stopColor={color} stopOpacity={0.35} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={size / 2} cy={size / 2} r={size / 2} fill={`url(#${id})`} />
      </Svg>
    </Animated.View>
  );
}

const siren: SceneFactory = (m, t) => {
  const slam = 1100;
  return {
    duration: 3900,
    bg: '#07070D',
    dim: 0.97,
    impacts: [{ at: slam, amp: 8 }],
    Render: function Siren({ clock, w, h, rand, cx, cy }: SceneProps) {
      const by = Math.max(70, cy - 210);
      const glints = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 16,
            speed: [500, 900],
            g: 300,
            k: 2.4,
            life: [500, 800],
            look: sparks([RED, '#FFFFFF'], 3),
          }),
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam + 20,
            count: 16,
            speed: [500, 900],
            g: 300,
            k: 2.4,
            life: [500, 800],
            look: sparks([BLUE, '#FFFFFF'], 3),
          }),
        ],
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Beams clock={clock} x={cx} y={by} size={Math.max(w, h) * 2.2} />
          <Wash clock={clock} color={RED} phase={0} peak={0.3} />
          <Wash clock={clock} color={BLUE} phase={1} peak={0.3} />
          <LightBar clock={clock} x={cx} y={by} />
          <Flash clock={clock} at={slam} color="#FFFFFF" peak={0.45} fall={300} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FFFFFF" size={480} width={6} />
          <Particles clock={clock} list={glints} />
          <Headline clock={clock} m={m} t={t} at={slam + 250} cy={cy} box={size * 1.1} ink="#FFFFFF" sub="#C8CCDA">
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color="#FFFFFF" font={t.font.display} glow="#FFFFFF66" />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

// ——— 180: darts ———

/** Annular sector between radii r0, r1 and angles a0, a1 (deg), centred on 0,0. */
function sector(r0: number, r1: number, a0: number, a1: number) {
  const p = (r: number, a: number) => `${(r * Math.cos((a * Math.PI) / 180)).toFixed(2)},${(r * Math.sin((a * Math.PI) / 180)).toFixed(2)}`;
  return `M${p(r0, a0)} L${p(r1, a0)} A${r1},${r1} 0 0 1 ${p(r1, a1)} L${p(r0, a1)} A${r0},${r0} 0 0 0 ${p(r0, a0)} Z`;
}

function Dartboard({ size }: { size: number }) {
  const parts = useMemo(
    () =>
      Array.from({ length: 20 }, (_, i) => {
        const a = -90 + i * 18;
        const even = i % 2 === 0;
        return {
          single: sector(0, 80, a - 9, a + 9),
          double: sector(80, 87, a - 9, a + 9),
          triple: sector(50, 57, a - 9, a + 9),
          base: even ? '#1B1B1B' : '#F1E6C8',
          ring: even ? '#D7263D' : '#1F8A4C',
        };
      }),
    [],
  );
  return (
    <Svg width={size} height={size} viewBox="-100 -100 200 200">
      <Circle cx={0} cy={0} r={99} fill="#151515" />
      <Circle cx={0} cy={0} r={99} fill="none" stroke="#2C2C2C" strokeWidth={2} />
      {parts.map((p, i) => (
        <G key={i}>
          <Path d={p.single} fill={p.base} />
          <Path d={p.double} fill={p.ring} />
          <Path d={p.triple} fill={p.ring} />
        </G>
      ))}
      <Circle cx={0} cy={0} r={9} fill="#1F8A4C" />
      <Circle cx={0} cy={0} r={4.5} fill="#D7263D" />
    </Svg>
  );
}

/** A dart pointing right, 96 × 26. */
function Dart({ flight }: { flight: string }) {
  return (
    <Svg width={96} height={26} viewBox="0 0 96 26">
      <Path d="M0,1 L24,13 L0,25 L9,13 Z" fill={flight} />
      <Path d="M2,4 L22,13 L2,22" stroke="#00000033" strokeWidth={1.2} fill="none" />
      <Rect x={14} y={11.5} width={26} height={3} fill="#3A3A3A" />
      <Rect x={38} y={8.5} width={30} height={9} rx={3} fill="#A0A6AD" />
      {[44, 49, 54, 59].map((x) => (
        <Rect key={x} x={x} y={8.5} width={1.6} height={9} fill="#6B7178" />
      ))}
      <Path d="M67,11.2 L96,13 L67,14.8 Z" fill="#E3E3E3" />
    </Svg>
  );
}

const darts: SceneFactory = (m, t) => {
  const hits = [520, 900, 1280];
  const slam = 1700;
  return {
    duration: 4100,
    bg: t.bg,
    impacts: [...hits.map((a) => ({ at: a, amp: 4 })), { at: slam, amp: 8 }],
    Render: function Darts({ clock, w, h, rand, cx, cy }: SceneProps) {
      const board = Math.min(210, cy * 0.9);
      const by = Math.max(board / 2 + 24, cy - 150 - board / 2 + 30);
      // Triple 20: the thin ring at the top of the board.
      const r = (53.5 / 200) * board;
      const L = 96;
      const flights = useMemo(() => {
        return hits.map((T, i) => {
          const px = cx + (i - 1) * (board * 0.045);
          const py = by - r;
          const sx = cx + 70 + i * 50;
          const sy = h + 60;
          const th = Math.atan2(py - sy, px - sx);
          const end = 0.72;
          const ex = px - (L / 2) * end * Math.cos(th);
          const ey = py - (L / 2) * end * Math.sin(th);
          const deg = (th * 180) / Math.PI;
          return {
            px,
            py,
            k: {
              t: [T - 281, T - 280, T, T + 60, T + 140, T + 240],
              x: [sx - L / 2, sx - L / 2, ex - L / 2, ex - L / 2, ex - L / 2, ex - L / 2],
              y: [sy - 13, sy - 13, ey - 13, ey - 13, ey - 13, ey - 13],
              r: [deg, deg, deg, deg + 6, deg - 3, deg],
              s: [1.8, 1.8, end, end, end, end],
              o: [0, 1, 1, 1, 1, 1],
            },
          };
        });
      }, [cx, by, r, h, board]);
      const wobble = useMemo(() => {
        const tt = [0, 200, 330];
        const rr = [0, 0, 0];
        const ss = [0.3, 1.08, 1];
        for (const T of hits) {
          tt.push(T, T + 70, T + 160, T + 280);
          rr.push(0, 2.5, -1.2, 0);
          ss.push(1, 1.03, 1, 1);
        }
        return { t: tt, r: rr, s: ss, o: [0, 1, 1, ...hits.flatMap(() => [1, 1, 1, 1])] };
      }, []);
      const fx = useMemo(
        () => [
          ...flights.flatMap((f, i) =>
            burst(rand, {
              x: f.px,
              y: f.py,
              at: hits[i],
              count: 9,
              speed: [160, 420],
              g: 700,
              k: 2.5,
              life: [300, 600],
              look: sparks(['#FFD54A', '#FFF1B8', t.acc], 2),
            }),
          ),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 34, speed: [400, 900], g: 900, k: 1.8, life: [1300, 1900], look: paper(party(t)) }),
        ],
        [flights, rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Rays clock={clock} at={slam} x={cx} y={cy} color={t.acc} opacity={0.16} />
          <Keyed clock={clock} style={at(cx, by, board, board)} k={wobble}>
            <Dartboard size={board} />
          </Keyed>
          {flights.map((f, i) => (
            <Shockwave key={i} clock={clock} at={hits[i]} x={f.px} y={f.py} color="#FFD54A" size={90} width={3} dur={400} />
          ))}
          {flights.map((f, i) => (
            <Keyed key={`d${i}`} clock={clock} style={{ position: 'absolute', left: 0, top: 0, width: L, height: 26 }} k={f.k}>
              <Dart flight={i === 1 ? '#FF4FA3' : t.acc} />
            </Keyed>
          ))}
          <Flash clock={clock} at={slam} color={t.acc} peak={0.3} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color={t.acc} size={480} width={7} />
          <Particles clock={clock} list={fx} />
          <Headline clock={clock} m={m} t={t} at={slam + 250} cy={cy} box={size * 1.1}>
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color={t.accInk} font={t.font.display} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

// ——— 214: hearts ———

const HEART =
  'M16 28.5 C16 28.5 1 19 1 9.5 C1 4.5 5 1 9.5 1 C12.5 1 14.8 2.6 16 4.8 C17.2 2.6 19.5 1 22.5 1 C27 1 31 4.5 31 9.5 C31 19 16 28.5 16 28.5 Z';

function Heart({ size, color, opacity = 1 }: { size: number; color: string; opacity?: number }) {
  return (
    <Svg width={size} height={size * 0.9} viewBox="0 0 32 29">
      <Path d={HEART} fill={color} fillOpacity={opacity} />
    </Svg>
  );
}

/** A heartbeat (boom-boom, rest) every `period` ms from `start`. */
function Beat({
  clock,
  start,
  period = 1000,
  amp = 0.14,
  children,
  style,
}: {
  clock: Clock;
  start: number;
  period?: number;
  amp?: number;
  children: ReactNode;
  style?: ViewStyle;
}) {
  const a = useAnimatedStyle(() => {
    const c = clock.value - start;
    if (c < 0) return { transform: [{ scale: 1 }] };
    const l = c % period;
    const s = interpolate(l, [0, 110, 220, 330, 470, period], [1, 1 + amp, 1, 1 + amp * 0.65, 1, 1], Extrapolation.CLAMP);
    return { transform: [{ scale: s }] };
  });
  return <Animated.View style={[style, a]}>{children}</Animated.View>;
}

const hearts: SceneFactory = (m, t) => {
  const pop = 450;
  const beat = 900;
  return {
    duration: 4000,
    bg: t.bg,
    impacts: [
      { at: beat, amp: 3 },
      { at: beat + 1000, amp: 2 },
      { at: beat + 2000, amp: 2 },
    ],
    Render: function Hearts({ clock, w, h, rand, cx, cy }: SceneProps) {
      const pinks = ['#FF4FA3', '#FF6DB5', '#E8338A', '#FF8FC7', '#FF3B5C'];
      const list = useMemo<Particle[]>(() => {
        const rising = Array.from({ length: 30 }, () => {
          const s = rand.range(16, 42);
          return {
            x: rand.range(0, w),
            y: h + 30,
            vx: 0,
            vy: -rand.range(170, 300),
            g: -50,
            k: 0.2,
            sway: rand.range(10, 26),
            swayHz: rand.range(0.5, 1.2),
            at: rand.range(0, 2700),
            life: rand.range(2800, 3600),
            fadeIn: 250,
            fadeOut: 800,
            rot: rand.range(-20, 20),
            spin: rand.range(-30, 30),
            w: s,
            h: s * 0.9,
            color: '#fff',
            shape: 'node',
            node: <Heart size={s} color={rand.pick(pinks)} />,
          } as Particle;
        });
        const flutter = burst(rand, {
          x: cx,
          y: cy,
          at: beat,
          count: 14,
          speed: [260, 560],
          g: 300,
          k: 2,
          life: [900, 1300],
          fadeOut: 500,
          look: (r) => {
            const s = r.range(12, 22);
            return { shape: 'node', w: s, h: s * 0.9, rot: r.range(-25, 25), node: <Heart size={s} color={r.pick(pinks)} /> };
          },
        });
        return [...rising, ...flutter];
      }, [rand, w, h, cx, cy]);
      const size = numberSize(m.n);
      const ink = t.tag;
      return (
        <>
          <Glow clock={clock} at={pop} x={cx} y={cy} color="#FF4FA3" size={420} opacity={isDark(t) ? 0.4 : 0.3} pulse={1} />
          <Keyed clock={clock} style={at(cx, cy + 6, 330, 300)} k={{ t: [pop, pop + 250], s: [0.5, 1], o: [0, 1] }}>
            <Beat clock={clock} start={beat} amp={0.08}>
              <Heart size={330} color="#FF4FA3" opacity={isDark(t) ? 0.16 : 0.14} />
            </Beat>
          </Keyed>
          <Shockwave clock={clock} at={beat} x={cx} y={cy} color="#FF4FA3" size={420} width={5} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={beat + 150} cy={cy} box={size * 1.1}>
            <Pop clock={clock} at={pop}>
              <Beat clock={clock} start={beat}>
                <BigNumber value={m.n} size={size} color={ink} font={t.font.display} />
              </Beat>
            </Pop>
          </Headline>
        </>
      );
    },
  };
};

// ——— 256, 512: pixels ———

const PIXEL = ['#FFE600', '#FF2A6D', '#05D9E8', '#3DFF6E', '#B48CFF', '#FF7A45'];

type Px = { x: number; y: number; vx: number; vy: number; g: number; at: number; life: number; size: number };

/** A square moving at 12 frames per second on a 4 px grid, like an old console. */
function Pixel({ clock, p, color }: { clock: Clock; p: Px; color: string }) {
  const a = useAnimatedStyle(() => {
    const ms = clock.value - p.at;
    if (ms < 0 || ms > p.life) return { opacity: 0, transform: [{ translateX: -9999 }, { translateY: 0 }] };
    const t = Math.floor((ms / 1000) * 12) / 12;
    const x = p.x + p.vx * t;
    const y = p.y + p.vy * t + 0.5 * p.g * t * t;
    return { opacity: 1, transform: [{ translateX: Math.round(x / 4) * 4 }, { translateY: Math.round(y / 4) * 4 }] };
  });
  return <Animated.View style={[{ position: 'absolute', left: 0, top: 0, width: p.size, height: p.size, backgroundColor: color }, a]} />;
}

/** Appears in a few hard steps (no easing), then holds. */
function StepPop({ clock, at: start, children }: { clock: Clock; at: number; children: ReactNode }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value - start;
    const s = c < 0 ? 0 : c < 85 ? 0.45 : c < 170 ? 1.3 : c < 255 ? 0.9 : c < 340 ? 1.06 : 1;
    return { opacity: c < 0 ? 0 : 1, transform: [{ scale: s }] };
  });
  return <Animated.View style={a}>{children}</Animated.View>;
}

/** Two hard white frames, the old-game hit flash. */
function HardFlash({ clock, at: start }: { clock: Clock; at: number }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value - start;
    return { opacity: (c >= 0 && c < 60) || (c >= 130 && c < 190) ? 0.55 : 0 };
  });
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: '#FFFFFF' }, a]} />;
}

const pixels: SceneFactory = (m, t) => {
  const slam = 1000;
  return {
    duration: 3800,
    bg: '#0F0F1B',
    dim: 0.97,
    impacts: [{ at: slam, amp: 6 }],
    Render: function Pixels({ clock, w, h, rand, cx, cy }: SceneProps) {
      const list = useMemo(() => {
        const fall = Array.from({ length: 44 }, () => ({
          p: {
            x: Math.round(rand.range(0, w) / 12) * 12,
            y: -20,
            vx: 0,
            vy: rand.range(260, 420),
            g: 0,
            at: rand.range(0, 3000),
            life: 4000,
            size: rand.pick([8, 12, 16]),
          },
          color: rand.pick(PIXEL),
        }));
        const pop = Array.from({ length: 30 }, () => {
          const a = rand.range(0, Math.PI * 2);
          const v = rand.range(250, 650);
          return {
            p: { x: cx, y: cy, vx: Math.cos(a) * v, vy: Math.sin(a) * v - 250, g: 1100, at: slam, life: 1600, size: rand.pick([8, 12]) },
            color: rand.pick(PIXEL),
          };
        });
        return [...fall, ...pop];
      }, [rand, w, cx, cy]);
      const grid = useMemo(() => {
        const rows: number[] = [];
        for (let y = 0; y < h; y += 4) rows.push(y);
        return rows;
      }, [h]);
      const size = numberSize(m.n);
      return (
        <>
          <Svg pointerEvents="none" width={w} height={h} style={StyleSheet.absoluteFill}>
            {grid.map((y) => (
              <Rect key={y} x={0} y={y} width={w} height={1} fill="#FFFFFF" fillOpacity={0.035} />
            ))}
          </Svg>
          {list.map((x, i) => (
            <Pixel key={i} clock={clock} p={x.p} color={x.color} />
          ))}
          <HardFlash clock={clock} at={slam} />
          <Headline clock={clock} m={m} t={t} at={slam + 300} cy={cy} box={size * 1.1} ink="#FFFFFF" sub="#B9B9D0">
            <StepPop clock={clock} at={slam - 260}>
              <View>
                <BigNumber value={m.n} size={size} color="#FF2A6D" font={t.font.display} style={{ position: 'absolute', left: 7, top: 7 }} />
                <BigNumber value={m.n} size={size} color="#FFE600" font={t.font.display} />
              </View>
            </StepPop>
          </Headline>
        </>
      );
    },
  };
};

// ——— 300: bowling ———

function Pin({ w }: { w: number }) {
  return (
    <Svg width={w} height={w * (60 / 22)} viewBox="0 0 22 60">
      <Path
        d="M11 0 C16 0 17 6 16 11 C15 15 13.2 17 13.2 20 C13.2 24 21 32 21 44 C21 54 17 60 11 60 C5 60 1 54 1 44 C1 32 8.8 24 8.8 20 C8.8 17 7 15 6 11 C5 6 6 0 11 0 Z"
        fill="#FFFFFF"
        stroke="#00000030"
        strokeWidth={1}
      />
      <Rect x={7.4} y={16.5} width={7.2} height={1.8} fill="#D7263D" />
      <Rect x={7.2} y={20} width={7.6} height={1.8} fill="#D7263D" />
      <Path d="M8 4 C7.5 6 7.5 9 8.5 11" stroke="#00000014" strokeWidth={1.5} fill="none" />
    </Svg>
  );
}

function Ball({ size, color }: { size: number; color: string }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 60 60">
      <Defs>
        <RadialGradient id="ballShine" cx="35%" cy="30%" r="70%">
          <Stop offset="0" stopColor="#FFFFFF" stopOpacity={0.45} />
          <Stop offset="0.5" stopColor="#FFFFFF" stopOpacity={0} />
        </RadialGradient>
      </Defs>
      <Circle cx={30} cy={30} r={29} fill={color} />
      <Circle cx={30} cy={30} r={29} fill="url(#ballShine)" />
      <Circle cx={24} cy={20} r={4} fill="#00000099" />
      <Circle cx={34} cy={19} r={4} fill="#00000099" />
      <Circle cx={30} cy={31} r={5} fill="#00000099" />
    </Svg>
  );
}

const bowling: SceneFactory = (m, t) => {
  const hit = 850;
  const slam = 1400;
  return {
    duration: 4000,
    bg: t.bg,
    impacts: [
      { at: hit, amp: 7 },
      { at: slam, amp: 6 },
    ],
    Render: function Bowling({ clock, w, h, rand, cx, cy }: SceneProps) {
      const py = Math.max(150, cy - 175);
      const pins = useMemo(() => {
        const out: { p: Throw; w: number }[] = [];
        // Back row first: front pins are drawn over them.
        for (let r = 3; r >= 0; r--) {
          const sc = 1 - r * 0.1;
          for (let j = 0; j <= r; j++) {
            const pw = 24 * sc;
            const ph = pw * (60 / 22);
            const x = cx + (j - r / 2) * 38 * sc;
            const y = py - r * 17 - ph / 2;
            out.push({
              w: pw,
              p: {
                x,
                y,
                vx: (j - r / 2) * rand.range(160, 340) + rand.range(-140, 140),
                vy: -rand.range(520, 980),
                g: 2300,
                k: 0.3,
                spin: rand.sign() * rand.range(300, 900),
                rot: 0,
                at: hit + (3 - r) * 0 + r * 35,
                w: pw,
                h: ph,
              },
            });
          }
        }
        return out;
      }, [rand, cx, py]);
      const lane = useMemo(() => {
        const top = py - 70;
        const tw = 90;
        const bw = Math.min(w * 0.9, 340);
        return `M${cx - tw},${top} L${cx + tw},${top} L${cx + bw},${h} L${cx - bw},${h} Z`;
      }, [cx, py, w, h]);
      const fx = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: py - 20,
            at: hit,
            count: 14,
            speed: [250, 600],
            g: 900,
            k: 2,
            life: [500, 900],
            look: sparks(['#FFFFFF', '#FFD54A', t.acc], 3),
          }),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 34, speed: [400, 900], g: 900, k: 1.8, life: [1300, 1900], look: paper(party(t)) }),
        ],
        [rand, cx, cy, py],
      );
      const ballSize = 64;
      const size = numberSize(m.n);
      const dark = isDark(t);
      return (
        <>
          <Svg pointerEvents="none" width={w} height={h} style={StyleSheet.absoluteFill}>
            <Defs>
              <LinearGradient id="lane" x1="0" y1="0" x2="0" y2="1">
                <Stop offset="0" stopColor={dark ? '#FFFFFF' : '#000000'} stopOpacity={0.02} />
                <Stop offset="1" stopColor={dark ? '#FFFFFF' : '#000000'} stopOpacity={0.07} />
              </LinearGradient>
            </Defs>
            <Path d={lane} fill="url(#lane)" stroke={t.line} strokeWidth={2} />
          </Svg>
          <Rays clock={clock} at={slam} x={cx} y={cy} color={t.acc} opacity={0.16} />
          <Shockwave clock={clock} at={hit} x={cx} y={py - 30} color={t.acc} size={300} width={5} />
          {pins.map((p, i) => (
            <Thrown key={i} clock={clock} p={p.p}>
              <Pin w={p.w} />
            </Thrown>
          ))}
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: cx - ballSize / 2, top: 0, width: ballSize, height: ballSize }}
            k={{
              t: [0, hit, hit + 300],
              y: [h + 40, py - 30, py - 140],
              x: [30, 0, -6],
              r: [0, -900, -1100],
              s: [1.6, 0.62, 0.4],
              o: [1, 1, 0],
            }}>
            <Ball size={ballSize} color={dark ? '#5B47FF' : '#3D2BD1'} />
          </Keyed>
          <Flash clock={clock} at={slam} color={t.acc} peak={0.3} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color={t.acc} size={480} width={7} />
          <Particles clock={clock} list={fx} />
          <Headline clock={clock} m={m} t={t} at={slam + 250} cy={cy} box={size * 1.1}>
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color={t.accInk} font={t.font.display} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

// ——— 321: countdown and lift-off ———

function Rocket() {
  return (
    <Svg width={60} height={130} viewBox="0 0 60 130">
      <Path d="M14,78 L2,104 L16,100 Z" fill="#FF4F5E" />
      <Path d="M46,78 L58,104 L44,100 Z" fill="#FF4F5E" />
      <Path d="M30,2 C46,16 50,44 46,100 L14,100 C10,44 14,16 30,2 Z" fill="#F2F4FA" />
      <Path d="M30,2 C38,9 43,19 45,30 L15,30 C17,19 22,9 30,2 Z" fill="#FF4F5E" />
      <Path d="M34,10 C39,20 41,44 39,92" stroke="#00000018" strokeWidth={5} fill="none" />
      <Circle cx={30} cy={52} r={9} fill="#2B3A67" stroke="#C9D2EA" strokeWidth={3} />
      <Circle cx={27} cy={49} r={2.5} fill="#FFFFFF" fillOpacity={0.7} />
      <Rect x={18} y={100} width={24} height={8} rx={2} fill="#7A8299" />
    </Svg>
  );
}

function Flame({ clock }: { clock: Clock }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const f = 1 + 0.22 * Math.sin(c * 0.07) + 0.12 * Math.sin(c * 0.19);
    return { transform: [{ translateY: 22 * (f - 1) }, { scaleY: f }, { scaleX: 1 - 0.08 * Math.sin(c * 0.11) }] };
  });
  return (
    <Animated.View style={[{ position: 'absolute', left: 14, top: 104, width: 32, height: 60 }, a]}>
      <Svg width={32} height={60} viewBox="0 0 32 60">
        <Path d="M3,0 L29,0 C30,22 22,40 16,60 C10,40 2,22 3,0 Z" fill="#FFB547" />
        <Path d="M9,0 L23,0 C24,14 20,26 16,38 C12,26 8,14 9,0 Z" fill="#FFF1B8" />
      </Svg>
    </Animated.View>
  );
}

const rocket: SceneFactory = (m, t) => {
  const counts = [200, 650, 1100];
  const lift = 1450;
  const fly = 1150;
  const slam = 2550;
  return {
    duration: 4200,
    bg: '#0B1026',
    dim: 0.97,
    impacts: [
      ...counts.map((c) => ({ at: c, amp: 4 })),
      { at: lift, amp: 6 },
      { at: lift + 160, amp: 4 },
      { at: lift + 320, amp: 3 },
      { at: slam, amp: 7 },
    ],
    Render: function Lift({ clock, w, h, rand, cx, cy }: SceneProps) {
      const y0 = h + 150;
      const y1 = -260;
      const acc = (2 * (y0 - y1)) / Math.pow(fly / 1000, 2);
      const ry = (ms: number) => {
        const s = Math.max(0, ms - lift) / 1000;
        return y0 - 0.5 * acc * s * s;
      };
      const stars = useMemo(
        () => Array.from({ length: 70 }, () => ({ x: rand.range(0, w), y: rand.range(0, h), r: rand.range(0.6, 2), o: rand.range(0.3, 0.9) })),
        [rand, w, h],
      );
      const list = useMemo<Particle[]>(() => {
        // The rocket's tail on screen at time `ms`.
        const tail = (ms: number) => ry(ms) + 50;
        const smoke = Array.from({ length: 16 }, (_, i) => {
          const at = lift + 120 + i * 50;
          return {
            x: cx + rand.range(-8, 8),
            y: tail(at) + 30,
            vx: rand.range(-70, 70),
            vy: rand.range(20, 90),
            g: 0,
            k: 1.5,
            at,
            life: 1500,
            fadeOut: 1200,
            grow: [0.6, 3] as [number, number],
            w: 34,
            h: 34,
            color: 'rgba(201,206,223,0.3)',
            shape: 'circle',
          } as Particle;
        });
        const fire = Array.from({ length: 22 }, (_, i) => {
          const at = lift + i * 40;
          return {
            x: cx + rand.range(-6, 6),
            y: Math.min(tail(at) + 50, h + 20),
            vx: rand.range(-60, 60),
            vy: rand.range(250, 450),
            g: 0,
            k: 2,
            at,
            life: 420,
            fadeOut: 300,
            w: 7,
            h: 7,
            color: rand.pick(['#FFB547', '#FF7A45', '#FFF1B8']),
            shape: 'circle',
          } as Particle;
        });
        const cloud = [-1, 1].flatMap((s) =>
          burst(rand, {
            x: cx,
            y: h + 10,
            at: lift,
            count: 9,
            dir: s === 1 ? -12 : -168,
            spread: 34,
            speed: [220, 520],
            g: -40,
            k: 2.2,
            life: [1600, 2100],
            fadeOut: 1100,
            grow: [1, 3],
            look: () => ({ shape: 'circle', color: 'rgba(185,190,208,0.55)', w: 46, h: 46 }),
          }),
        );
        const party_ = burst(rand, {
          x: cx,
          y: cy,
          at: slam,
          count: 30,
          speed: [400, 900],
          g: 900,
          k: 1.8,
          life: [1200, 1700],
          look: paper(party(t)),
        });
        return [...smoke, ...cloud, ...fire, ...party_];
        // ry depends only on these.
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, [rand, cx, cy, h]);
      const path = useMemo(() => {
        const ts = Array.from({ length: 9 }, (_, i) => lift + (fly * i) / 8);
        return { t: [0, ...ts], y: [y0 - 65, ...ts.map((ms) => ry(ms) - 65)] };
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, [h]);
      const size = numberSize(m.n);
      return (
        <>
          <Svg pointerEvents="none" width={w} height={h} style={StyleSheet.absoluteFill}>
            {stars.map((s, i) => (
              <Circle key={i} cx={s.x} cy={s.y} r={s.r} fill="#FFF6D8" fillOpacity={s.o} />
            ))}
          </Svg>
          {counts.map((c, i) => (
            <Keyed
              key={i}
              clock={clock}
              style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0 }}
              k={{ t: [c + 300, c + 420], o: [1, 0], s: [1, 1.5] }}>
              <View style={{ position: 'absolute', top: cy - 110, left: 0, right: 0, alignItems: 'center' }}>
                <Slam clock={clock} at={c} from={2.4} fall={150}>
                  <BigNumber value={3 - i} size={200} color="#FFFFFF" font={t.font.display} />
                </Slam>
              </View>
            </Keyed>
          ))}
          {counts.map((c, i) => (
            <Shockwave key={`s${i}`} clock={clock} at={c} x={cx} y={cy} color="#FFFFFF" size={320} width={4} dur={450} />
          ))}
          <Particles clock={clock} list={list} />
          <Keyed clock={clock} style={{ position: 'absolute', left: cx - 30, top: 0, width: 60, height: 130 }} k={path}>
            <Flame clock={clock} />
            <Rocket />
          </Keyed>
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#FFE9A8" opacity={0.18} />
          <Flash clock={clock} at={slam} color="#FFFFFF" peak={0.3} />
          <Headline clock={clock} m={m} t={t} at={slam + 250} cy={cy} box={size * 1.1} ink="#FFFFFF" sub="#C9D2EA">
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color="#FFFFFF" font={t.font.display} glow="#FFE9A8" />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

// ——— 365: tear-off calendar ———

function Page({ day, font, bold }: { day: number; font: string; bold: string }) {
  const tr = useT();
  return (
    <View
      style={{ width: 140, height: 156, borderRadius: 18, backgroundColor: '#FFFFFF', borderWidth: 1, borderColor: '#00000018', overflow: 'hidden' }}>
      <View style={{ height: 42, backgroundColor: '#E5484D', alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ fontFamily: bold, fontSize: 17, letterSpacing: 2, color: '#FFFFFF' }}>{tr('celebration.december')}</Text>
      </View>
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ fontFamily: font, fontSize: 76, lineHeight: 84, letterSpacing: -3, color: '#121216' }}>{day}</Text>
      </View>
    </View>
  );
}

const calendar: SceneFactory = (m, t) => {
  const first = 250;
  const step = 95;
  const pages = 12;
  const slam = first + pages * step + 200;
  return {
    duration: 4000,
    bg: t.bg,
    impacts: [
      { at: first, amp: 2 },
      { at: slam, amp: 8 },
    ],
    Render: function Calendar({ clock, rand, cx, cy }: SceneProps) {
      const torn = useMemo(
        () =>
          Array.from({ length: pages }, (_, i) => {
            const s = rand.sign();
            return {
              day: 19 + i,
              p: {
                x: cx,
                y: cy,
                vx: s * rand.range(320, 700),
                vy: -rand.range(550, 950),
                g: 2800,
                k: 0.4,
                spin: s * rand.range(300, 800),
                rot: 0,
                at: first + i * step,
                w: 140,
                h: 156,
              },
            };
          }).reverse(),
        [rand, cx, cy],
      );
      const list = useMemo(
        () => burst(rand, { x: cx, y: cy, at: slam, count: 36, speed: [400, 900], g: 900, k: 1.8, life: [1300, 1900], look: paper(party(t)) }),
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Rays clock={clock} at={slam} x={cx} y={cy} color={t.acc} opacity={0.16} />
          <Keyed clock={clock} style={at(cx, cy, 140, 156)} k={{ t: [0, 180, slam - 140, slam - 20], s: [0.4, 1, 1, 0.5], o: [0, 1, 1, 0] }}>
            <Page day={31} font={t.font.display} bold={t.font.bold} />
          </Keyed>
          {torn.map((p) => (
            <Keyed key={p.day} clock={clock} k={{ t: [0, 180], o: [0, 1] }}>
              <Thrown clock={clock} p={p.p}>
                <Page day={p.day} font={t.font.display} bold={t.font.bold} />
              </Thrown>
            </Keyed>
          ))}
          {/* The binder rings stay with the block. */}
          <Keyed clock={clock} style={at(cx, cy - 78, 104, 20)} k={{ t: [0, 180, slam - 140, slam - 20], o: [0, 1, 1, 0] }}>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingHorizontal: 20 }}>
              <View style={{ width: 12, height: 20, borderRadius: 6, backgroundColor: '#3A3A44' }} />
              <View style={{ width: 12, height: 20, borderRadius: 6, backgroundColor: '#3A3A44' }} />
            </View>
          </Keyed>
          <Flash clock={clock} at={slam} color={t.acc} peak={0.3} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color={t.acc} size={480} width={7} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 250} cy={cy} box={size * 1.1}>
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color={t.accInk} font={t.font.display} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

// ——— 404: glitch ———

const GLITCH_ON = 250;

/** How glitchy the screen is at `c` ms: a storm at first, then calm with two short relapses. */
const glitchAt = (c: number) => {
  'worklet';
  if (c < GLITCH_ON) return 0;
  if ((c > 2300 && c < 2460) || (c > 3080 && c < 3180)) return 0.85;
  return interpolate(c, [GLITCH_ON, 1100, 1600], [1, 0.8, 0.1], Extrapolation.CLAMP);
};

function GlitchBand({
  clock,
  i,
  bands,
  bh,
  width,
  children,
}: {
  clock: Clock;
  i: number;
  bands: number;
  bh: number;
  width: number;
  children: ReactNode;
}) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const g = glitchAt(c);
    const step = Math.floor(c / 60);
    const on = hash(step * 7.13 + i * 1.7) > 1 - g * 0.65;
    const dx = on ? (hash(step * 13.7 + i * 3.1) - 0.5) * 70 * g : 0;
    const flick = c < GLITCH_ON ? 0 : c < 700 ? (hash(step * 3.3) > 0.3 ? 1 : 0.15) : 1;
    return { opacity: flick, transform: [{ translateX: dx }] };
  });
  return (
    <Animated.View style={[{ position: 'absolute', left: 0, top: i * bh, width, height: bh + (i === bands - 1 ? 2 : 0.5), overflow: 'hidden' }, a]}>
      <View style={{ position: 'absolute', left: 0, top: -i * bh, width, height: bh * bands, alignItems: 'center', justifyContent: 'center' }}>
        {children}
      </View>
    </Animated.View>
  );
}

function Ghost({ clock, dir, children }: { clock: Clock; dir: 1 | -1; children: ReactNode }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const g = glitchAt(c);
    const step = Math.floor(c / 70);
    const dx = dir * (3 + hash(step * 5.1 + dir) * 14 * g);
    const dy = (hash(step * 9.7 - dir) - 0.5) * 8 * g;
    return { opacity: c < GLITCH_ON ? 0 : 0.8, transform: [{ translateX: dx }, { translateY: dy }] };
  });
  return <Animated.View style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center' }, a]}>{children}</Animated.View>;
}

/** Coloured bars flashing at random places while it glitches. */
function Noise({ clock, i, w, h }: { clock: Clock; i: number; w: number; h: number }) {
  const colors = ['#FF2A6D', '#05D9E8', '#FFFFFF'];
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const g = glitchAt(c);
    const step = Math.floor(c / 70);
    const on = hash(step * 31.1 + i * 4.7) < g * 0.55;
    const x = hash(step * 11.3 + i) * w - 60;
    const y = hash(step * 17.9 + i * 2.3) * h;
    const sx = 0.2 + hash(step * 23.1 + i) * 1.6;
    return { opacity: on ? 0.75 : 0, transform: [{ translateX: x }, { translateY: y }, { scaleX: sx }] };
  });
  return (
    <Animated.View style={[{ position: 'absolute', left: 0, top: 0, width: 120, height: 3 + (i % 4) * 4, backgroundColor: colors[i % 3] }, a]} />
  );
}

function ScanBar({ clock, w, h }: { clock: Clock; w: number; h: number }) {
  const a = useAnimatedStyle(() => ({ transform: [{ translateY: ((clock.value % 1700) / 1700) * (h + 160) - 120 }] }));
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: 0, top: 0, width: w, height: 120 }, a]}>
      <Svg width={w} height={120}>
        <Defs>
          <LinearGradient id="scan" x1="0" y1="0" x2="0" y2="1">
            <Stop offset="0" stopColor="#FFFFFF" stopOpacity={0} />
            <Stop offset="0.85" stopColor="#FFFFFF" stopOpacity={0.07} />
            <Stop offset="1" stopColor="#FFFFFF" stopOpacity={0} />
          </LinearGradient>
        </Defs>
        <Rect x={0} y={0} width={w} height={120} fill="url(#scan)" />
      </Svg>
    </Animated.View>
  );
}

const glitch: SceneFactory = (m, t) => ({
  duration: 4000,
  bg: '#050508',
  dim: 0.98,
  impacts: [
    { at: GLITCH_ON, amp: 6 },
    { at: 2300, amp: 3 },
    { at: 3080, amp: 2 },
  ],
  Render: function Glitch({ clock, w, h, cy }: SceneProps) {
    const size = numberSize(m.n);
    const box = size * 1.1;
    const bands = 7;
    const bh = box / bands;
    const lines = useMemo(() => Array.from({ length: Math.ceil(h / 4) }, (_, i) => i * 4), [h]);
    const number = (color: string) => <BigNumber value={m.n} size={size} color={color} font={t.font.display} />;
    return (
      <>
        {Array.from({ length: 12 }, (_, i) => (
          <Noise key={i} clock={clock} i={i} w={w} h={h} />
        ))}
        <ScanBar clock={clock} w={w} h={h} />
        <Headline clock={clock} m={m} t={t} at={1300} cy={cy} box={box} ink="#F5F5F7" sub="#9A9AA6">
          <View style={{ width: w, height: box }}>
            <Ghost clock={clock} dir={-1}>
              {number('#05D9E8')}
            </Ghost>
            <Ghost clock={clock} dir={1}>
              {number('#FF2A6D')}
            </Ghost>
            {Array.from({ length: bands }, (_, i) => (
              <GlitchBand key={i} clock={clock} i={i} bands={bands} bh={bh} width={w}>
                {number('#F5F5F7')}
              </GlitchBand>
            ))}
          </View>
        </Headline>
        <Svg pointerEvents="none" width={w} height={h} style={StyleSheet.absoluteFill}>
          {lines.map((y) => (
            <Rect key={y} x={0} y={y} width={w} height={1.2} fill="#000000" fillOpacity={0.35} />
          ))}
        </Svg>
      </>
    );
  },
});

// ——— 1664: beer ———

function Mug({ glass, flip }: { glass: string; flip?: boolean }) {
  return (
    <Svg width={92} height={112} viewBox="0 0 92 112">
      <G transform={flip ? 'translate(92,0) scale(-1,1)' : undefined}>
        <Path d="M66,38 C86,38 88,46 88,62 C88,80 84,88 66,88" stroke={glass} strokeWidth={8} fill="none" />
        <Rect x={8} y={22} width={60} height={86} rx={10} fill="#F2A900" />
        <Rect x={14} y={30} width={10} height={70} rx={5} fill="#FFFFFF" fillOpacity={0.25} />
        <Rect x={8} y={22} width={60} height={86} rx={10} fill="none" stroke={glass} strokeWidth={4} />
        {[14, 26, 38, 50, 62].map((x, i) => (
          <Circle key={x} cx={x} cy={i % 2 ? 16 : 20} r={i % 2 ? 12 : 10} fill="#FFFDF4" />
        ))}
      </G>
    </Svg>
  );
}

function Bob({ clock, i, x, r }: { clock: Clock; i: number; x: number; r: number }) {
  const a = useAnimatedStyle(() => ({ transform: [{ translateY: Math.sin(clock.value * 0.005 + i * 1.3) * 4 }] }));
  return (
    <Animated.View
      style={[{ position: 'absolute', left: x - r, top: -r * 0.9, width: r * 2, height: r * 2, borderRadius: r, backgroundColor: '#FFFDF4' }, a]}
    />
  );
}

const beer: SceneFactory = (m, t) => {
  const clink = 1150;
  const slam = 1500;
  return {
    duration: 4000,
    bg: t.bg,
    impacts: [
      { at: clink, amp: 5 },
      { at: slam, amp: 7 },
    ],
    Render: function Beer({ clock, w, h, rand, cx, cy }: SceneProps) {
      const dark = isDark(t);
      const top = Math.min(h - 120, Math.max(h * 0.66, cy + 240));
      const body = h - top;
      const my = Math.max(70, cy - 215);
      const foam = useMemo(() => {
        const out: { x: number; r: number }[] = [];
        for (let x = -10; x < w + 20; x += rand.range(20, 30)) out.push({ x, r: rand.range(13, 22) });
        return out;
      }, [rand, w]);
      const bubbles = useMemo<Particle[]>(
        () =>
          Array.from({ length: 34 }, () => {
            const s = rand.range(4, 10);
            return {
              x: rand.range(0, w),
              y: body + 10,
              vx: 0,
              vy: -rand.range(60, 160),
              g: -90,
              k: 0.3,
              sway: rand.range(3, 9),
              swayHz: rand.range(0.8, 2),
              at: rand.range(0, 3600),
              life: 3000,
              w: s,
              h: s,
              color: '#FFF6D8',
              shape: 'circle',
              fadeIn: 200,
            } as Particle;
          }),
        [rand, w, body],
      );
      const fx = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: my - 30,
            at: clink,
            count: 18,
            dir: -90,
            spread: 140,
            speed: [250, 600],
            g: 1200,
            k: 1,
            life: [700, 1100],
            look: (r) => {
              const s = r.range(5, 11);
              return { shape: 'circle', color: r.pick(['#F2A900', '#FFD54A', '#FFFDF4']), w: s, h: s };
            },
          }),
          ...burst(rand, {
            x: cx,
            y: my - 30,
            at: clink,
            count: 10,
            speed: [200, 420],
            g: 200,
            k: 2.5,
            life: [350, 600],
            look: sparks(['#FFFFFF', '#FFF1B8'], 2),
          }),
        ],
        [rand, cx, my],
      );
      const glass = dark ? '#FFFFFFCC' : '#9C9C94';
      const size = numberSize(m.n);
      const ink = dark ? '#F2A900' : '#B26A00';
      return (
        <>
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: -20, top, width: w + 40, height: body + 40 }}
            k={{ t: [100, 700, 1300, 1700], y: [body + 40, body * 0.45, body * 0.06, 0] }}>
            <View style={{ position: 'absolute', left: 0, top: 0, width: w + 40, height: body + 40, overflow: 'hidden' }}>
              <Svg width={w + 40} height={body + 40} style={StyleSheet.absoluteFill}>
                <Defs>
                  <LinearGradient id="beer" x1="0" y1="0" x2="0" y2="1">
                    <Stop offset="0" stopColor="#FFC531" />
                    <Stop offset="1" stopColor="#C77700" />
                  </LinearGradient>
                </Defs>
                <Rect x={0} y={0} width={w + 40} height={body + 40} fill="url(#beer)" />
              </Svg>
              <Particles clock={clock} list={bubbles} />
            </View>
            {foam.map((f, i) => (
              <Bob key={i} clock={clock} i={i} x={f.x} r={f.r} />
            ))}
          </Keyed>
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#FFC531" opacity={0.2} />
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: 0, top: my - 56, width: 92, height: 112 }}
            k={{ t: [150, clink, clink + 80, clink + 260], x: [-110, cx - 82, cx - 92, cx - 88], r: [0, 16, 8, 11] }}>
            <Mug glass={glass} flip />
          </Keyed>
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: 0, top: my - 56, width: 92, height: 112 }}
            k={{ t: [150, clink, clink + 80, clink + 260], x: [w + 20, cx - 10, cx, cx - 4], r: [0, -16, -8, -11] }}>
            <Mug glass={glass} />
          </Keyed>
          <Shockwave clock={clock} at={clink} x={cx} y={my - 20} color="#FFD54A" size={220} width={4} dur={500} />
          <Particles clock={clock} list={fx} />
          <Flash clock={clock} at={slam} color="#FFC531" peak={0.25} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FFC531" size={480} width={7} />
          <Headline clock={clock} m={m} t={t} at={slam + 250} cy={cy} box={size * 1.1}>
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color={ink} font={t.font.display} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

export const SCENES: Record<number, SceneFactory> = {
  101: dalmatians,
  112: siren,
  180: darts,
  214: hearts,
  256: pixels,
  512: pixels,
  300: bowling,
  321: rocket,
  365: calendar,
  404: glitch,
  1664: beer,
};
