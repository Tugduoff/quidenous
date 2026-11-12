/** Bespoke epic scenes: 42 (galaxy), 69 (wink), 314 (pie), 1001 (night), 1024 (pixels), 2048 (tiles). */
import { type ReactNode, useMemo } from 'react';
import { StyleSheet, Text, View, type ViewStyle } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, Ellipse, Line, LinearGradient, Path, RadialGradient, Rect, Stop } from 'react-native-svg';

import {
  BigNumber,
  burst,
  type Clock,
  Flash,
  Glow,
  paper,
  type Particle,
  Particles,
  Pop,
  type Rand,
  Rays,
  Reveal,
  Shockwave,
  Slam,
  sparks,
} from '../engine';
import { GOLD, Headline, numberSize, type SceneFactory, type SceneProps } from '../scene';

const clamp01 = (v: number) => {
  'worklet';
  return Math.min(1, Math.max(0, v));
};
const easeOut = (x: number) => {
  'worklet';
  return 1 - Math.pow(1 - x, 3);
};
const easeInOut = (x: number) => {
  'worklet';
  return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
};

const ABS: ViewStyle = { position: 'absolute', left: 0, top: 0 };

// ——— Shared pieces ———

type Star = { x: number; y: number; r: number };

/** A layer of stars twinkling together (one Svg, one animated opacity). */
function StarLayer({
  clock,
  stars,
  w,
  h,
  phase,
  period,
  color,
}: {
  clock: Clock;
  stars: Star[];
  w: number;
  h: number;
  phase: number;
  period: number;
  color: string;
}) {
  const style = useAnimatedStyle(() => ({ opacity: 0.3 + 0.7 * (0.5 + 0.5 * Math.sin((clock.value / period) * Math.PI * 2 + phase)) }));
  return (
    <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, style]}>
      <Svg width={w} height={h}>
        {stars.map((s, i) => (
          <Circle key={i} cx={s.x} cy={s.y} r={s.r} fill={color} />
        ))}
      </Svg>
    </Animated.View>
  );
}

/** Three layers of stars twinkling out of phase. */
function StarField({
  clock,
  rand,
  w,
  h,
  count,
  color = '#FFF6D8',
  maxY = 1,
}: {
  clock: Clock;
  rand: Rand;
  w: number;
  h: number;
  count: number;
  color?: string;
  maxY?: number;
}) {
  const layers = useMemo(
    () =>
      [0, 1, 2].map(() =>
        Array.from({ length: Math.round(count / 3) }, () => ({
          x: rand.range(0, w),
          y: rand.range(0, h * maxY),
          r: rand.f() < 0.15 ? rand.range(1.4, 2.2) : rand.range(0.5, 1.2),
        })),
      ),
    [rand, w, h, count, maxY],
  );
  return (
    <>
      {layers.map((stars, i) => (
        <StarLayer key={i} clock={clock} stars={stars} w={w} h={h} phase={i * 2.1} period={1300 + i * 450} color={color} />
      ))}
    </>
  );
}

/**
 * Something orbiting (cx, cy): its angle turns at `spin` rad/s, its radius eases from r0 to r1 after `at`.
 * The orbit is an ellipse (`tilt` squashes it) turned by `tiltRot` radians.
 */
type Orbit = { r0: number; r1: number; a: number; spin: number; at: number; dur: number; size: number };
function Orbiter({
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
    const b = easeOut(clamp01((ms - at) / dur));
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
function Window({ clock, from, to, children, style }: { clock: Clock; from: number; to: number; children: ReactNode; style?: ViewStyle }) {
  const anim = useAnimatedStyle(() => ({ opacity: clock.value >= from && clock.value < to ? 1 : 0 }));
  return <Animated.View style={[style, anim]}>{children}</Animated.View>;
}

/** Fades from 1 to 0 between `from` and `to`. */
function FadeAway({ clock, from, to, children, style }: { clock: Clock; from: number; to: number; children: ReactNode; style?: ViewStyle }) {
  const anim = useAnimatedStyle(() => ({ opacity: interpolate(clock.value, [from, to], [1, 0], Extrapolation.CLAMP) }));
  return <Animated.View style={[style, anim]}>{children}</Animated.View>;
}

// ——— 42: a galaxy swirls, the answer comes out of its core ———

/** The galaxy turns as a whole at this rate (rad/s); its arms bend as `ARM_TWIST` radians from core to rim. */
const SPIN = 0.45;
const ARM_TWIST = 3.4;
const TILT = 0.55;
const TILT_ROT = -0.32;

/** The soft glowing arms behind the stars: one Svg turning, squashed into the galaxy's ellipse. */
function Arms({ clock, cx, cy, R, color }: { clock: Clock; cx: number; cy: number; R: number; color: string }) {
  const S = R * 2 + 40;
  // Each arm: soft dust clouds (radial gradients) shrinking toward the rim.
  const blobs = useMemo(
    () =>
      [0, 1, 2].flatMap((arm) =>
        Array.from({ length: 36 }, (_, i) => {
          const u = i / 35;
          const a = (arm / 3) * Math.PI * 2 + u * ARM_TWIST;
          const r = 14 + u * R;
          return { x: S / 2 + r * Math.cos(a), y: S / 2 + r * Math.sin(a), r: 34 - 16 * u, accent: i % 9 === 4 && u < 0.7 };
        }),
      ),
    [R, S],
  );
  const anim = useAnimatedStyle(() => {
    const ms = clock.value;
    const b = easeOut(clamp01(ms / 1900));
    return {
      opacity: b,
      transform: [
        { rotate: `${(TILT_ROT * 180) / Math.PI}deg` },
        { scaleY: TILT },
        { rotate: `${((SPIN * ms) / 1000) * (180 / Math.PI)}deg` },
        { scale: 0.1 + 0.9 * b },
      ],
    };
  });
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: cx - S / 2, top: cy - S / 2, width: S, height: S }, anim]}>
      <Svg width={S} height={S}>
        <Defs>
          <RadialGradient id="dust42" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#A98BFF" stopOpacity={0.24} />
            <Stop offset="1" stopColor="#A98BFF" stopOpacity={0} />
          </RadialGradient>
          <RadialGradient id="dustAcc42" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#FFFFFF" stopOpacity={0.18} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        {blobs.map((b, i) => (
          <Circle key={i} cx={b.x} cy={b.y} r={b.r} fill={b.accent ? 'url(#dustAcc42)' : 'url(#dust42)'} />
        ))}
      </Svg>
    </Animated.View>
  );
}

const g42: SceneFactory = (m, t) => {
  const birth = 1700;
  return {
    duration: 6000,
    bg: '#05040F',
    dim: 0.97,
    impacts: [{ at: birth + 380, amp: 4 }],
    Render: function Galaxy({ clock, w, h, rand, cx, cy }: SceneProps) {
      const colors = [t.acc, '#B48CFF', '#5AC8FA', '#FFFFFF', '#C2A3FF', '#7FB2FF'];
      const R = Math.min(w * 0.5, 210);
      const dots = useMemo(() => {
        const arms = 3;
        return Array.from({ length: 120 }, (_, i) => {
          const arm = i % arms;
          const u = Math.pow(rand.f(), 0.8);
          const r1 = 14 + u * R + rand.range(-8, 8);
          const a = (arm / arms) * Math.PI * 2 + u * ARM_TWIST + rand.range(-0.28, 0.28) * (1 - u * 0.4);
          const spin = SPIN + 0.15 * (50 / (r1 + 50));
          const core = u < 0.18;
          return {
            o: { r0: r1 * 0.08, r1, a, spin, at: rand.range(0, 900) + u * 500, dur: 1500, size: core ? rand.range(3.5, 6) : rand.range(2, 4.6) },
            color: core ? rand.pick(['#FFFFFF', '#F2E8FF', t.acc]) : rand.pick(colors),
          };
        });
      }, [rand, R, colors]);
      const size = numberSize(m.n);
      return (
        <>
          <StarField clock={clock} rand={rand} w={w} h={h} count={90} />
          <Glow clock={clock} at={0} x={cx} y={cy} color="#6A3FD0" size={R * 2.6} opacity={0.55} pulse={0.4} />
          <Glow clock={clock} at={300} x={cx} y={cy} color="#B48CFF" size={R * 1.1} opacity={0.6} pulse={0.7} />
          <Arms clock={clock} cx={cx} cy={cy} R={R} color={t.acc} />
          {dots.map((d, i) => (
            <Orbiter
              key={i}
              clock={clock}
              o={d.o}
              cx={cx}
              cy={cy}
              tilt={TILT}
              tiltRot={TILT_ROT}
              style={{ borderRadius: d.o.size / 2, backgroundColor: d.color }}
            />
          ))}
          <Glow clock={clock} at={birth} x={cx} y={cy} color={t.acc} size={260} opacity={0.4} pulse={0.9} />
          <Flash clock={clock} at={birth + 380} color="#C2A3FF" peak={0.25} fall={600} />
          <Shockwave clock={clock} at={birth + 380} x={cx} y={cy} color="#C2A3FF" size={460} width={3} dur={900} />
          <Rocket clock={clock} from={3000} to={5600} w={w} h={h} />
          <Headline clock={clock} m={m} t={t} at={birth + 700} cy={cy} box={size * 1.1} ink="#F4F0FF" sub="#A9A3C9">
            <Emerge clock={clock} at={birth}>
              <BigNumber value={m.n} size={size} color="#FFFFFF" font={t.font.display} glow={t.acc} />
            </Emerge>
          </Headline>
        </>
      );
    },
  };
};

/** Grows out of a point while unwinding a quarter turn, with a small overshoot. */
function Emerge({ clock, at, children }: { clock: Clock; at: number; children: ReactNode }) {
  const anim = useAnimatedStyle(() => {
    const c = clock.value;
    const s = interpolate(c, [at, at + 380, at + 520, at + 680], [0.05, 1.12, 0.97, 1], Extrapolation.CLAMP);
    const r = interpolate(c, [at, at + 380], [-100, 0], Extrapolation.CLAMP);
    return { opacity: interpolate(c, [at, at + 160], [0, 1], Extrapolation.CLAMP), transform: [{ rotate: `${r}deg` }, { scale: s }] };
  });
  return <Animated.View style={anim}>{children}</Animated.View>;
}

/** A little rocket crossing the bottom of the sky, with a trail of sparks. */
function Rocket({ clock, from, to, w, h }: { clock: Clock; from: number; to: number; w: number; h: number }) {
  const [x0, y0, x1, y1] = [-50, h * 0.8, w + 50, h * 0.66];
  const angle = (Math.atan2(y1 - y0, x1 - x0) * 180) / Math.PI;
  const anim = useAnimatedStyle(() => {
    const p = clamp01((clock.value - from) / (to - from));
    const on = clock.value >= from && p < 1;
    const bob = Math.sin(p * Math.PI * 6) * 4;
    return {
      opacity: on ? 1 : 0,
      transform: [{ translateX: x0 + (x1 - x0) * p - 22 }, { translateY: y0 + (y1 - y0) * p - 11 + bob }, { rotate: `${angle}deg` }],
    };
  });
  const trail = useMemo<Particle[]>(
    () =>
      Array.from({ length: 16 }, (_, i) => {
        const p = (i + 0.5) / 16;
        return {
          x: x0 + (x1 - x0) * p - 18,
          y: y0 + (y1 - y0) * p + 2,
          vx: -40,
          vy: 15,
          k: 1,
          at: from + p * (to - from),
          life: 700,
          fadeOut: 650,
          grow: [1, 0.3],
          w: 5,
          h: 5,
          color: i % 2 ? '#FFB547' : '#FFF1B8',
          shape: 'circle',
        } as Particle;
      }),
    [x0, y0, x1, y1, from, to],
  );
  return (
    <>
      <Particles clock={clock} list={trail} />
      <Animated.View pointerEvents="none" style={[ABS, { width: 44, height: 22 }, anim]}>
        <Svg width={44} height={22} viewBox="0 0 44 22">
          <Path d="M8 11 L2 4 L12 7 Z M8 11 L2 18 L12 15 Z" fill="#FF4FA3" />
          <Path d="M8 6 Q30 2 42 11 Q30 20 8 16 Z" fill="#EDEAF7" />
          <Circle cx={27} cy={11} r={3.2} fill="#5AC8FA" stroke="#2B2350" strokeWidth={1.4} />
          <Path d="M8 8 L4 11 L8 14 Z" fill="#FFB547" />
        </Svg>
      </Animated.View>
    </>
  );
}

// ——— 69: a smiley winks, the number turns upside down and stays 69 ———

const PINKS = ['#FF4FA3', '#FF8FB1', '#FFB38A', '#FFD1DC', '#FFC94D'];

const g69: SceneFactory = (m, t) => {
  const slam = 650;
  const turn = 2350;
  return {
    duration: 5400,
    bg: t.bg,
    impacts: [
      { at: slam, amp: 6 },
      { at: turn + 520, amp: 4 },
    ],
    Render: function Wink({ clock, w, h, rand, cx, cy }: SceneProps) {
      const fy = Math.max(cy - 200, 120);
      const list = useMemo(
        () => [
          ...burst(rand, { x: cx, y: cy, at: slam, count: 26, speed: [350, 800], g: 900, k: 1.8, life: [1200, 1700], look: paper(PINKS) }),
          ...burst(rand, { x: cx, y: cy, at: turn + 520, count: 40, speed: [450, 1000], g: 900, k: 1.7, life: [1500, 2200], look: paper(PINKS) }),
          ...burst(rand, {
            x: -10,
            y: h * 0.7,
            at: turn + 560,
            count: 14,
            dir: -55,
            spread: 30,
            speed: [800, 1300],
            g: 900,
            k: 1.3,
            life: [1800, 2300],
            look: paper(PINKS),
          }),
          ...burst(rand, {
            x: w + 10,
            y: h * 0.7,
            at: turn + 560,
            count: 14,
            dir: -125,
            spread: 30,
            speed: [800, 1300],
            g: 900,
            k: 1.3,
            life: [1800, 2300],
            look: paper(PINKS),
          }),
        ],
        [rand, w, h, cx, cy],
      );
      const size = numberSize(m.n) + 20;
      return (
        <>
          <Glow clock={clock} at={turn + 520} x={cx} y={cy} color="#FF4FA3" size={360} opacity={0.3} pulse={0.6} />
          <Shockwave clock={clock} at={turn + 520} x={cx} y={cy} color="#FF8FB1" size={420} width={5} />
          <Face clock={clock} x={cx} y={fy} size={160} winks={[1250, 3200]} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={turn + 650} cy={cy} box={size * 1.1}>
            <Slam clock={clock} at={slam}>
              <Turn clock={clock} at={turn}>
                <BigNumber value={m.n} size={size} color="#FF4FA3" font={t.font.display} />
              </Turn>
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

/** Half a turn with a wind-up and a small overshoot. */
function Turn({ clock, at, children }: { clock: Clock; at: number; children: ReactNode }) {
  const anim = useAnimatedStyle(() => {
    const r = interpolate(clock.value, [at, at + 140, at + 460, at + 560, at + 680], [0, -12, 192, 176, 180], Extrapolation.CLAMP);
    const s = interpolate(clock.value, [at, at + 260, at + 520, at + 680], [1, 0.86, 1.08, 1], Extrapolation.CLAMP);
    return { transform: [{ rotate: `${r}deg` }, { scale: s }] };
  });
  return <Animated.View style={anim}>{children}</Animated.View>;
}

/** A drawn smiley: pops in, raises an eyebrow and winks at each time in `winks`. */
function Face({ clock, x, y, size, winks }: { clock: Clock; x: number; y: number; size: number; winks: number[] }) {
  const s = size / 170;
  const [w1, w2] = winks;
  // The wink: the right eye closes into an arc, the left brow shoots up, the head tilts.
  const wink = (c: number) => {
    'worklet';
    const a = interpolate(c, [w1, w1 + 90, w1 + 700, w1 + 800], [0, 1, 1, 0], Extrapolation.CLAMP);
    const b = interpolate(c, [w2, w2 + 90, w2 + 520, w2 + 620], [0, 1, 1, 0], Extrapolation.CLAMP);
    return Math.max(a, b);
  };
  const head = useAnimatedStyle(() => ({ transform: [{ rotate: `${-9 * wink(clock.value)}deg` }] }));
  const open = useAnimatedStyle(() => ({ transform: [{ scaleY: 1 - 0.92 * wink(clock.value) }], opacity: 1 - wink(clock.value) * 0.9 }));
  const shut = useAnimatedStyle(() => ({ opacity: wink(clock.value) }));
  const brow = useAnimatedStyle(() => ({ transform: [{ translateY: -10 * s * wink(clock.value) }, { rotate: `${-12 * wink(clock.value)}deg` }] }));
  const ink = '#2B1A0E';
  return (
    <View pointerEvents="none" style={{ position: 'absolute', left: x - size / 2, top: y - size / 2, width: size, height: size }}>
      <Pop clock={clock} at={120}>
        <Animated.View style={[{ width: size, height: size }, head]}>
          <Svg width={size} height={size} viewBox="0 0 170 170">
            <Defs>
              <RadialGradient id="face69" cx="40%" cy="35%" r="70%">
                <Stop offset="0" stopColor="#FFE07A" />
                <Stop offset="1" stopColor="#FFB938" />
              </RadialGradient>
            </Defs>
            <Circle cx={85} cy={85} r={82} fill="url(#face69)" />
            <Circle cx={40} cy={108} r={14} fill="#FF7FA8" opacity={0.45} />
            <Circle cx={140} cy={114} r={13} fill="#FF7FA8" opacity={0.45} />
            <Path d="M48 110 Q92 142 132 100" stroke={ink} strokeWidth={9} fill="none" strokeLinecap="round" />
            {/* Right brow, calm. */}
            <Path d="M100 42 Q116 36 130 44" stroke={ink} strokeWidth={7} fill="none" strokeLinecap="round" />
          </Svg>
          {/* Left eye, wide open, its brow flying up. */}
          <View style={{ position: 'absolute', left: 44 * s, top: 56 * s, width: 24 * s, height: 32 * s }}>
            <Svg width={24 * s} height={32 * s} viewBox="0 0 24 32">
              <Ellipse cx={12} cy={16} rx={11} ry={15} fill={ink} />
              <Circle cx={15} cy={10} r={3.5} fill="#FFFFFF" />
            </Svg>
          </View>
          <Animated.View style={[{ position: 'absolute', left: 34 * s, top: 30 * s, width: 44 * s, height: 20 * s }, brow]}>
            <Svg width={44 * s} height={20 * s} viewBox="0 0 44 20">
              <Path d="M4 14 Q22 2 40 10" stroke={ink} strokeWidth={7} fill="none" strokeLinecap="round" />
            </Svg>
          </Animated.View>
          {/* Right eye: open, or shut into an arc while winking. */}
          <Animated.View style={[{ position: 'absolute', left: 104 * s, top: 56 * s, width: 24 * s, height: 32 * s }, open]}>
            <Svg width={24 * s} height={32 * s} viewBox="0 0 24 32">
              <Ellipse cx={12} cy={16} rx={11} ry={15} fill={ink} />
              <Circle cx={15} cy={10} r={3.5} fill="#FFFFFF" />
            </Svg>
          </Animated.View>
          <Animated.View style={[{ position: 'absolute', left: 98 * s, top: 62 * s, width: 36 * s, height: 22 * s }, shut]}>
            <Svg width={36 * s} height={22 * s} viewBox="0 0 36 22">
              <Path d="M4 14 Q18 2 32 14" stroke={ink} strokeWidth={7} fill="none" strokeLinecap="round" />
            </Svg>
          </Animated.View>
        </Animated.View>
      </Pop>
    </View>
  );
}

// ——— 314: a pie fills like a pie chart, the digits of π spiral around it ———

const PI = '3,14159265358979323846264338327950';
const BAKERY = ['#F2A541', '#E07A2E', '#FFD9A0', '#C98A45', '#8C4A21'];

const g314: SceneFactory = (m, t) => {
  const fill: [number, number] = [350, 1900];
  const slam = 2500;
  return {
    duration: 6000,
    bg: '#1C120C',
    dim: 0.97,
    impacts: [
      { at: fill[1], amp: 3 },
      { at: slam, amp: 7 },
    ],
    Render: function Pie({ clock, w, h, rand, cx, cy }: SceneProps) {
      const R = 84;
      const py = cy - 110;
      const ny = cy + 90;
      const digits = useMemo(
        () =>
          PI.split('').map((d, i) => ({
            d,
            o: {
              r0: R + 4,
              r1: R + 34 + (i % 2) * 6,
              a: -Math.PI / 2 + (i / PI.length) * Math.PI * 2,
              spin: 0.22,
              at: fill[0] + 250 + i * 55,
              dur: 650,
              size: 22,
            },
          })),
        [],
      );
      const list = useMemo(
        () => [
          // Steam off the warm pie.
          ...Array.from(
            { length: 10 },
            (_, i) =>
              ({
                x: cx + rand.range(-R * 0.5, R * 0.5),
                y: py - R * 0.3,
                vx: rand.range(-10, 10),
                vy: rand.range(-70, -45),
                k: 0.2,
                at: fill[1] + i * 300,
                life: 2000,
                fadeIn: 400,
                fadeOut: 1000,
                sway: 8,
                swayHz: 0.6,
                grow: [0.5, 1.6],
                w: 40,
                h: 40,
                color: '#fff',
                shape: 'node',
                node: <Steam />,
              }) as Particle,
          ),
          ...burst(rand, { x: cx, y: ny, at: slam, count: 34, speed: [350, 850], g: 900, k: 1.8, life: [1300, 1900], look: paper(BAKERY) }),
        ],
        [rand, cx, py, ny],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Glow clock={clock} at={fill[1]} x={cx} y={py} color="#FFB547" size={R * 3.4} opacity={0.35} pulse={0.5} />
          <Shockwave clock={clock} at={fill[1]} x={cx} y={py} color="#FFD9A0" size={R * 3.2} width={4} />
          <View pointerEvents="none" style={{ position: 'absolute', left: cx - R, top: py - R, width: R * 2, height: R * 2 }}>
            <Pop clock={clock} at={0}>
              <PieDish clock={clock} R={R} from={fill[0]} to={fill[1]} />
            </Pop>
          </View>
          {digits.map((d, i) => (
            <Orbiter key={i} clock={clock} o={d.o} cx={cx} cy={py}>
              <Text style={{ fontFamily: t.font.display, fontSize: i === 0 ? 20 : 16, color: i === 0 ? '#FFD9A0' : '#E8B887' }}>{d.d}</Text>
            </Orbiter>
          ))}
          <Flash clock={clock} at={slam} color="#FFB547" peak={0.25} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 300} cy={ny} box={size * 1.1} ink="#FFE9C8" sub="#C9A27E">
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color="#FFB547" font={t.font.display} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

/** The pie: an empty tin, filling clockwise from noon (two half-discs turning behind two clipping halves), then the crust and lattice. */
function PieDish({ clock, R, from, to }: { clock: Clock; R: number; from: number; to: number }) {
  const progress = (c: number) => {
    'worklet';
    return easeInOut(clamp01((c - from) / (to - from)));
  };
  const right = useAnimatedStyle(() => ({ transform: [{ rotate: `${Math.min(progress(clock.value), 0.5) * 360}deg` }] }));
  const left = useAnimatedStyle(() => {
    const p = progress(clock.value);
    return { opacity: p > 0.5 ? 1 : 0, transform: [{ rotate: `${Math.max(0, p - 0.5) * 360}deg` }] };
  });
  const top = useAnimatedStyle(() => ({ opacity: interpolate(clock.value, [to - 100, to + 250], [0, 1], Extrapolation.CLAMP) }));
  const D = R * 2;
  const filling = '#F2A541';
  // Lattice strips: chords of the inner circle, turned 45°.
  const lattice = useMemo(() => {
    const r = R - 12;
    const c = Math.SQRT1_2;
    const turn = (x: number, y: number) => [R + (x - y) * c, R + (x + y) * c];
    return [-0.6, -0.2, 0.2, 0.6].flatMap((k, i) => {
      const x = k * r;
      const half = Math.sqrt(r * r - x * x);
      return [
        { p: [...turn(x, -half), ...turn(x, half)], color: '#E8A85E', key: `v${i}` },
        { p: [...turn(-half, x), ...turn(half, x)], color: '#D9934A', key: `h${i}` },
      ];
    });
  }, [R]);
  const crimps = useMemo(() => Array.from({ length: 26 }, (_, i) => (i / 26) * Math.PI * 2), []);
  return (
    <View style={{ width: D, height: D }}>
      <Svg width={D} height={D} style={ABS}>
        <Circle cx={R} cy={R} r={R} fill="#3B2618" />
        <Circle cx={R} cy={R} r={R - 12} fill="#2A1A10" />
      </Svg>
      <View style={{ position: 'absolute', left: R, top: 0, width: R, height: D, overflow: 'hidden' }}>
        <Animated.View style={[{ position: 'absolute', left: -R, top: 0, width: D, height: D }, right]}>
          <View
            style={{
              position: 'absolute',
              left: 0,
              top: 0,
              width: R,
              height: D,
              backgroundColor: filling,
              borderTopLeftRadius: R,
              borderBottomLeftRadius: R,
            }}
          />
        </Animated.View>
      </View>
      <View style={{ position: 'absolute', left: 0, top: 0, width: R, height: D, overflow: 'hidden' }}>
        <Animated.View style={[{ position: 'absolute', left: 0, top: 0, width: D, height: D }, left]}>
          <View
            style={{
              position: 'absolute',
              left: R,
              top: 0,
              width: R,
              height: D,
              backgroundColor: filling,
              borderTopRightRadius: R,
              borderBottomRightRadius: R,
            }}
          />
        </Animated.View>
      </View>
      <Animated.View style={[ABS, top]}>
        <Svg width={D} height={D}>
          {lattice.map((l) => (
            <Line key={l.key} x1={l.p[0]} y1={l.p[1]} x2={l.p[2]} y2={l.p[3]} stroke={l.color} strokeWidth={9} strokeLinecap="round" />
          ))}
        </Svg>
      </Animated.View>
      <Svg width={D} height={D} style={ABS}>
        <Circle cx={R} cy={R} r={R - 8} stroke="#C98A45" strokeWidth={12} fill="none" />
        {crimps.map((a, i) => (
          <Circle key={i} cx={R + Math.cos(a) * (R - 8)} cy={R + Math.sin(a) * (R - 8)} r={7} fill={i % 2 ? '#D99B4E' : '#CF9046'} />
        ))}
      </Svg>
    </View>
  );
}

/** A soft puff of steam. */
function Steam() {
  return (
    <Svg width={40} height={40}>
      <Defs>
        <RadialGradient id="steam314" cx="50%" cy="50%" r="50%">
          <Stop offset="0" stopColor="#FFF4E6" stopOpacity={0.35} />
          <Stop offset="1" stopColor="#FFF4E6" stopOpacity={0} />
        </RadialGradient>
      </Defs>
      <Circle cx={20} cy={20} r={20} fill="url(#steam314)" />
    </Svg>
  );
}

// ——— 2048: tiles slide and merge all the way up ———

const TILE: Record<number, [string, string]> = {
  2: ['#EEE4DA', '#776E65'],
  4: ['#EDE0C8', '#776E65'],
  8: ['#F2B179', '#FFFFFF'],
  16: ['#F59563', '#FFFFFF'],
  32: ['#F67C5F', '#FFFFFF'],
  64: ['#F65E3B', '#FFFFFF'],
  128: ['#EDCF72', '#FFFFFF'],
  256: ['#EDCC61', '#FFFFFF'],
  512: ['#EDC850', '#FFFFFF'],
  1024: ['#EDC53F', '#FFFFFF'],
  2048: ['#EDC22E', '#FFFFFF'],
};

function Tile({ v, size, font }: { v: number; size: number; font: string }) {
  const [bg, ink] = TILE[v];
  const digits = String(v).length;
  const fs = size * (digits <= 2 ? 0.5 : digits === 3 ? 0.4 : 0.31);
  return (
    <View style={{ width: size, height: size, borderRadius: size * 0.12, backgroundColor: bg, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ fontFamily: font, fontSize: fs, color: ink, letterSpacing: -1, includeFontPadding: false }}>{v}</Text>
    </View>
  );
}

const g2048: SceneFactory = (m, t) => {
  // Merge times: the chain speeds up, then the last two 1024 take their time.
  const merges: number[] = [];
  let at = 500;
  for (let i = 0; i < 9; i++) {
    at += Math.max(150, 330 - i * 26);
    merges.push(at);
  }
  const last = merges[8] + 650;
  const slam = last + 330;
  return {
    duration: 6200,
    bg: t.bg,
    impacts: [...merges.slice(5).map((a) => ({ at: a, amp: 1.5 })), { at: last, amp: 4 }, { at: slam, amp: 9 }],
    Render: function Tiles({ clock, w, h, rand, cx, cy }: SceneProps) {
      const c = 64;
      const gap = 10;
      const B = c * 4 + gap * 5;
      const bx = cx - B / 2;
      const by = cy - B / 2;
      const cell = (r: number, col: number) => ({ x: bx + gap + col * (c + gap), y: by + gap + r * (c + gap) });
      const M = cell(1, 1);
      const sources = [cell(1, 3), cell(3, 1), cell(1, 0), cell(0, 1)];
      const font = t.font.display;
      const list = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 44,
            speed: [450, 1000],
            g: 900,
            k: 1.7,
            life: [1500, 2200],
            look: paper([...GOLD, '#F67C5F', '#F2B179']),
          }),
          ...burst(rand, {
            x: -10,
            y: h * 0.75,
            at: slam + 80,
            count: 16,
            dir: -58,
            spread: 28,
            speed: [900, 1400],
            g: 900,
            k: 1.3,
            life: [1800, 2300],
            look: paper(['#EDC22E', '#F59563', '#EEE4DA']),
          }),
          ...burst(rand, {
            x: w + 10,
            y: h * 0.75,
            at: slam + 80,
            count: 16,
            dir: -122,
            spread: 28,
            speed: [900, 1400],
            g: 900,
            k: 1.3,
            life: [1800, 2300],
            look: paper(['#EDC22E', '#F59563', '#EEE4DA']),
          }),
        ],
        [rand, w, h, cx, cy],
      );
      return (
        <>
          <FadeAway clock={clock} from={last + 80} to={slam - 40} style={StyleSheet.absoluteFill}>
            <Pop clock={clock} at={0} style={{ position: 'absolute', left: bx, top: by }}>
              <View style={{ width: B, height: B, borderRadius: 12, backgroundColor: '#BBADA0' }}>
                {Array.from({ length: 16 }, (_, i) => {
                  const p = cell(Math.floor(i / 4), i % 4);
                  return (
                    <View
                      key={i}
                      style={{
                        position: 'absolute',
                        left: p.x - bx,
                        top: p.y - by,
                        width: c,
                        height: c,
                        borderRadius: 8,
                        backgroundColor: '#CDC1B4',
                      }}
                    />
                  );
                })}
                {(
                  [
                    [0, 3, 4],
                    [3, 3, 2],
                    [2, 2, 4],
                    [3, 0, 2],
                  ] as const
                ).map(([r, col, v]) => {
                  const p = cell(r, col);
                  return (
                    <View key={`${r}${col}`} style={{ position: 'absolute', left: p.x - bx, top: p.y - by }}>
                      <Tile v={v} size={c} font={font} />
                    </View>
                  );
                })}
              </View>
            </Pop>
            {/* The tile in the merge cell, one per value, each popping in when it is born. */}
            {[2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048].map((v, i) => {
              const from = i === 0 ? 300 : i <= 9 ? merges[i - 1] : last;
              const to = i < 9 ? merges[i] : i === 9 ? last : 1e9;
              return (
                <Window key={v} clock={clock} from={from} to={to} style={{ position: 'absolute', left: M.x, top: M.y }}>
                  <Pop clock={clock} at={from}>
                    <Tile v={v} size={c} font={font} />
                  </Pop>
                </Window>
              );
            })}
            {/* The twin tiles: appear on a neighbour, then slide into the merge cell. */}
            {[2, 4, 8, 16, 32, 64, 128, 256, 512, 1024].map((v, i) => {
              const end = i < 9 ? merges[i] : last;
              const slide = i < 9 ? 110 : 260;
              const src = i < 9 ? sources[i % 4] : cell(1, 3);
              return (
                <Twin key={v} clock={clock} v={v} size={c} font={font} from={src} to={M} spawn={end - slide - 170} start={end - slide} end={end} />
              );
            })}
          </FadeAway>
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#EDC22E" opacity={0.3} />
          <Glow clock={clock} at={slam} x={cx} y={cy} color="#EDC22E" size={380} opacity={0.4} pulse={0.7} />
          <Flash clock={clock} at={slam} color="#FFE082" peak={0.4} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#EDC22E" size={560} width={8} dur={700} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 280} cy={cy} box={196}>
            <Slam clock={clock} at={slam} from={2.6}>
              <Tile v={2048} size={180} font={font} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

/** A tile that pops in on a neighbour cell at `spawn`, slides into `to` between `start` and `end`, then is gone. */
function Twin({
  clock,
  v,
  size,
  font,
  from,
  to,
  spawn,
  start,
  end,
}: {
  clock: Clock;
  v: number;
  size: number;
  font: string;
  from: { x: number; y: number };
  to: { x: number; y: number };
  spawn: number;
  start: number;
  end: number;
}) {
  const [fx, fy, tx, ty] = [from.x, from.y, to.x, to.y];
  const anim = useAnimatedStyle(() => {
    const c = clock.value;
    if (c < spawn || c >= end) return { opacity: 0, transform: [{ translateX: -9999 }, { translateY: 0 }, { scale: 1 }] };
    const p = clamp01((c - start) / (end - start));
    const e = p * p;
    const s = interpolate(c, [spawn, spawn + 120, spawn + 170], [0.2, 1.08, 1], Extrapolation.CLAMP);
    return { opacity: 1, transform: [{ translateX: fx + (tx - fx) * e }, { translateY: fy + (ty - fy) * e }, { scale: s }] };
  });
  return (
    <Animated.View style={[ABS, anim]}>
      <Tile v={v} size={size} font={font} />
    </Animated.View>
  );
}

// ——— 1024: 8-bit — pixels fall in steps, a loading bar fills up to 1 Ko ———

const PIXEL = ['#FFE600', '#FF2A6D', '#05D9E8', '#3DFF6E', '#B48CFF', '#FF7A45'];

const g1024: SceneFactory = (m, t) => {
  const segs = 16;
  const load: [number, number] = [300, 1800];
  const slam = 2150;
  return {
    duration: 5400,
    bg: '#0F0F1B',
    dim: 0.98,
    impacts: [
      { at: load[1], amp: 2 },
      { at: slam, amp: 8 },
    ],
    Render: function Pixels({ clock, w, h, rand, cx, cy }: SceneProps) {
      const blocks = useMemo(
        () =>
          Array.from({ length: 46 }, () => {
            const size = rand.pick([8, 12, 12, 16]);
            return {
              x: Math.round(rand.range(0, w - size) / 8) * 8,
              y0: -size - Math.round(rand.range(0, 6)) * 16,
              at: rand.range(0, 3600),
              step: rand.pick([70, 90, 110]),
              size,
              color: rand.pick(PIXEL),
            };
          }),
        [rand, w],
      );
      const list = useMemo(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 36,
            speed: [350, 900],
            g: 1000,
            k: 1.6,
            life: [1100, 1700],
            look: (r) => {
              const s = r.pick([6, 8, 10]);
              return { shape: 'rect', color: r.pick(PIXEL), w: s, h: s, rot: 0, spin: 0 };
            },
          }),
        [rand, cx, cy],
      );
      const barW = 240;
      const barY = cy - 150;
      const segW = (barW - 6 - 2 * (segs + 1)) / segs;
      const size = numberSize(m.n);
      return (
        <>
          <Scanlines w={w} h={h} />
          {blocks.map((b, i) => (
            <StepBlock key={i} clock={clock} b={b} h={h} />
          ))}
          <View pointerEvents="none" style={{ position: 'absolute', left: cx - barW / 2, top: barY, width: barW }}>
            <View style={{ height: 30, borderWidth: 3, borderColor: '#FFFFFF', flexDirection: 'row', padding: 2, gap: 2 }}>
              {Array.from({ length: segs }, (_, i) => (
                <Segment
                  key={i}
                  clock={clock}
                  at={load[0] + ((load[1] - load[0]) * (i + 1)) / segs}
                  blinkFrom={load[1]}
                  w={segW}
                  color={i < segs - 1 ? '#3DFF6E' : '#FFE600'}
                />
              ))}
            </View>
            <View style={{ height: 22, marginTop: 8 }}>
              <FadeAway clock={clock} from={load[1]} to={load[1] + 1} style={{ position: 'absolute', left: 0, right: 0 }}>
                <Text style={{ fontFamily: 'monospace', fontSize: 14, color: '#9C9CC0', textAlign: 'center', letterSpacing: 2 }}>CHARGEMENT…</Text>
              </FadeAway>
              <Window clock={clock} from={load[1]} to={1e9} style={{ position: 'absolute', left: 0, right: 0 }}>
                <Text style={{ fontFamily: 'monospace', fontSize: 14, color: '#FFE600', textAlign: 'center', letterSpacing: 2 }}>1 Ko · 100 %</Text>
              </Window>
            </View>
          </View>
          <Flash clock={clock} at={slam} color="#FFFFFF" peak={0.35} rise={0} fall={180} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#05D9E8" size={460} width={6} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 280} cy={cy} box={size * 1.1} ink="#FFFFFF" sub="#9C9CC0">
            <Slam clock={clock} at={slam} from={2.4}>
              <BigNumber
                value={m.n}
                size={size}
                color="#FFE600"
                font={t.font.display}
                style={{ letterSpacing: 2, textShadowColor: '#FF2A6D', textShadowOffset: { width: 6, height: 6 }, textShadowRadius: 0 }}
              />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

/** One segment of the loading bar: snaps on at `at`, then the full bar blinks three times. */
function Segment({ clock, at, blinkFrom, w, color }: { clock: Clock; at: number; blinkFrom: number; w: number; color: string }) {
  const anim = useAnimatedStyle(() => {
    const c = clock.value;
    if (c < at) return { opacity: 0 };
    const k = Math.floor((c - blinkFrom) / 110);
    return { opacity: c >= blinkFrom && k < 6 && k % 2 === 0 ? 0.25 : 1 };
  });
  return <Animated.View style={[{ width: w, height: '100%', backgroundColor: color }, anim]} />;
}

/** A pixel falling one cell at a time, like an old console. */
function StepBlock({
  clock,
  b,
  h,
}: {
  clock: Clock;
  b: { x: number; y0: number; at: number; step: number; size: number; color: string };
  h: number;
}) {
  const { x, y0, at, step, size } = b;
  const anim = useAnimatedStyle(() => {
    const c = clock.value - at;
    if (c < 0) return { opacity: 0, transform: [{ translateX: x }, { translateY: -50 }] };
    const y = y0 + Math.floor(c / step) * 16;
    return { opacity: y > h ? 0 : 1, transform: [{ translateX: x }, { translateY: y }] };
  });
  return <Animated.View pointerEvents="none" style={[ABS, { width: size, height: size, backgroundColor: b.color }, anim]} />;
}

/** CRT scanlines over the whole screen (static). */
function Scanlines({ w, h }: { w: number; h: number }) {
  const lines = Math.ceil(h / 4);
  return (
    <Svg pointerEvents="none" width={w} height={h} style={ABS}>
      {Array.from({ length: lines }, (_, i) => (
        <Rect key={i} x={0} y={i * 4} width={w} height={1.5} fill="#FFFFFF" opacity={0.035} />
      ))}
    </Svg>
  );
}

// ——— 1001: a night of tales — moon rising, lanterns, shooting stars ———

const g1001: SceneFactory = (m, t) => {
  const reveal = 1500;
  return {
    duration: 6400,
    bg: '#070B1E',
    dim: 0.98,
    impacts: [{ at: reveal + 250, amp: 2 }],
    Render: function Night({ clock, w, h, rand, cx, cy }: SceneProps) {
      const lanterns = useMemo<Particle[]>(
        () =>
          Array.from({ length: 15 }, () => {
            const s = rand.range(0.55, 1.25);
            const lw = Math.round(52 * s);
            const lh = Math.round(52 * s);
            return {
              x: rand.range(20, w - 20),
              y: h + 30 + rand.range(0, 100),
              vx: rand.range(-8, 8),
              vy: -rand.range(80, 140) * (0.6 + s * 0.4),
              k: 0.001,
              at: rand.range(-1200, 2600),
              life: 9000,
              fadeIn: 700,
              sway: rand.range(6, 14),
              swayHz: rand.range(0.2, 0.45),
              rot: rand.range(-6, 6),
              w: lw,
              h: lh,
              color: '#FFB547',
              shape: 'node',
              node: <Lantern w={lw} h={lh} />,
            } as Particle;
          }),
        [rand, w, h],
      );
      const shooting = useMemo<Particle[]>(
        () =>
          [900, 2700, 4400].map((at) => ({
            x: rand.range(w * 0.05, w * 0.5),
            y: rand.range(h * 0.04, h * 0.2),
            vx: 760,
            vy: 300,
            k: 0.001,
            at,
            life: 600,
            fadeIn: 60,
            fadeOut: 380,
            w: 80,
            h: 2,
            color: '#FFFFFF',
            shape: 'streak',
          })),
        [rand, w, h],
      );
      const sparkle = useMemo(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: reveal + 200,
            count: 22,
            speed: [120, 380],
            g: 60,
            k: 2,
            life: [1200, 1900],
            fadeOut: 700,
            look: sparks(GOLD, 2.5),
          }),
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Sky w={w} h={h} />
          <StarField clock={clock} rand={rand} w={w} h={h} count={96} maxY={0.75} />
          <Moon clock={clock} w={w} h={h} />
          <Particles clock={clock} list={shooting} />
          <Glow clock={clock} at={reveal} x={cx} y={cy} color="#FFB547" size={360} opacity={0.32} pulse={0.5} />
          <Particles clock={clock} list={lanterns} />
          <Particles clock={clock} list={sparkle} />
          <Headline clock={clock} m={m} t={t} at={reveal + 450} cy={cy} box={size * 1.1} ink="#FFF6D8" sub="#B9B3D9">
            <Pop clock={clock} at={reveal}>
              <BigNumber value={m.n} size={size} color="#FFE9A8" font={t.font.display} glow="#FFB547" />
            </Pop>
          </Headline>
        </>
      );
    },
  };
};

function Sky({ w, h }: { w: number; h: number }) {
  return (
    <Svg pointerEvents="none" width={w} height={h} style={ABS}>
      <Defs>
        <LinearGradient id="sky1001" x1="0" y1="0" x2="0" y2="1">
          <Stop offset="0" stopColor="#050818" />
          <Stop offset="0.65" stopColor="#141A44" />
          <Stop offset="1" stopColor="#3A2350" />
        </LinearGradient>
      </Defs>
      <Rect x={0} y={0} width={w} height={h} fill="url(#sky1001)" />
    </Svg>
  );
}

/** A crescent rising with its halo. */
function Moon({ clock, w, h }: { clock: Clock; w: number; h: number }) {
  const S = 150;
  const [x, y0, y1] = [w * 0.74, h * 0.34, h * 0.13];
  const anim = useAnimatedStyle(() => {
    const p = easeOut(clamp01(clock.value / 2600));
    return {
      opacity: clamp01(clock.value / 700),
      transform: [{ translateX: x - S / 2 }, { translateY: y0 + (y1 - y0) * p - S / 2 }, { rotate: `${-18 + 10 * p}deg` }],
    };
  });
  const R = 30;
  const c = S / 2;
  return (
    <Animated.View pointerEvents="none" style={[ABS, { width: S, height: S }, anim]}>
      <Svg width={S} height={S}>
        <Defs>
          <RadialGradient id="moonHalo" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#FFE9A8" stopOpacity={0.45} />
            <Stop offset="0.4" stopColor="#FFE9A8" stopOpacity={0.12} />
            <Stop offset="1" stopColor="#FFE9A8" stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={c} cy={c} r={c} fill="url(#moonHalo)" />
        <Path d={`M${c},${c - R} A${R},${R} 0 1 0 ${c},${c + R} A${R * 1.3},${R * 1.3} 0 0 1 ${c},${c - R} Z`} fill="#FFE9A8" />
      </Svg>
    </Animated.View>
  );
}

/** A paper sky lantern, lit from inside, in its own halo. The box is twice the lantern's width. */
function Lantern({ w, h }: { w: number; h: number }) {
  const lw = w / 2;
  const lh = (lw * 36) / 26;
  return (
    <View style={{ width: w, height: h, alignItems: 'center', justifyContent: 'center' }}>
      <Svg width={w} height={h} style={ABS}>
        <Defs>
          <RadialGradient id="lanternHalo" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#FFB547" stopOpacity={0.35} />
            <Stop offset="1" stopColor="#FFB547" stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={w / 2} cy={h / 2} r={Math.min(w, h) / 2} fill="url(#lanternHalo)" />
      </Svg>
      <Svg width={lw} height={lh} viewBox="0 0 26 36">
        <Defs>
          <RadialGradient id="lantern" cx="50%" cy="85%" r="80%">
            <Stop offset="0" stopColor="#FFF4C2" />
            <Stop offset="0.45" stopColor="#FFB547" />
            <Stop offset="1" stopColor="#D9541E" />
          </RadialGradient>
        </Defs>
        <Path d="M3 2 Q13 -1 23 2 L20 30 Q13 32 6 30 Z" fill="url(#lantern)" />
        <Ellipse cx={13} cy={31} rx={4} ry={2.2} fill="#FFF4C2" />
      </Svg>
    </View>
  );
}

export const SCENES: Record<number, SceneFactory> = { 42: g42, 69: g69, 314: g314, 1001: g1001, 1024: g1024, 2048: g2048 };
