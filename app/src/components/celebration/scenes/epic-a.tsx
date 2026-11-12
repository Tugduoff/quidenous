/** Epic scenes: 666 (cracks and embers), 777 (slot machine), 1000/2000 (MILLE), 1337 (matrix), 1515 (Marignan), 1789 (Bastille). */
import { type ReactNode, useMemo } from 'react';
import { Platform, StyleSheet, Text, View, type ViewStyle } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, G, LinearGradient, Path, Pattern, Polygon, RadialGradient, Rect, Stop } from 'react-native-svg';

import {
  BigNumber,
  burst,
  type Clock,
  Flash,
  firework,
  Glow,
  paper,
  type Particle,
  Particles,
  Pop,
  type Rand,
  rain,
  Rays,
  Reveal,
  RollNumber,
  Shockwave,
  Slam,
  sparks,
} from '../engine';
import { GOLD, Headline, numberSize, type SceneFactory, type SceneProps } from '../scene';

const MONO = Platform.OS === 'web' ? 'ui-monospace, Menlo, Consolas, monospace' : 'monospace';

// ——— Helpers ———

type Keys = { x?: number[]; y?: number[]; rot?: number[]; s?: number[]; sx?: number[]; sy?: number[]; o?: number[] };

/** Keyframes on the clock: each track is interpolated over `times` (clamped). */
function useKeys(clock: Clock, times: number[], k: Keys) {
  return useAnimatedStyle(() => {
    const c = clock.value;
    const x = k.x ? interpolate(c, times, k.x, Extrapolation.CLAMP) : 0;
    const y = k.y ? interpolate(c, times, k.y, Extrapolation.CLAMP) : 0;
    const rot = k.rot ? interpolate(c, times, k.rot, Extrapolation.CLAMP) : 0;
    const s = k.s ? interpolate(c, times, k.s, Extrapolation.CLAMP) : 1;
    const sx = k.sx ? interpolate(c, times, k.sx, Extrapolation.CLAMP) : s;
    const sy = k.sy ? interpolate(c, times, k.sy, Extrapolation.CLAMP) : s;
    const o = k.o ? interpolate(c, times, k.o, Extrapolation.CLAMP) : 1;
    return { opacity: o, transform: [{ translateX: x }, { translateY: y }, { rotate: `${rot}deg` }, { scaleX: sx }, { scaleY: sy }] };
  });
}

function Keyed({ clock, times, k, style, children }: { clock: Clock; times: number[]; k: Keys; style?: ViewStyle; children?: ReactNode }) {
  const a = useKeys(clock, times, k);
  return <Animated.View style={[style, a]}>{children}</Animated.View>;
}

function clamp01(v: number) {
  'worklet';
  return Math.min(1, Math.max(0, v));
}

/** Ease-out with a small overshoot. */
function backOut(x: number, c1 = 1.2) {
  'worklet';
  const c3 = c1 + 1;
  return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2);
}

const glowText = (color: string, radius = 24) => ({ textShadowColor: color, textShadowRadius: radius, textShadowOffset: { width: 0, height: 0 } });

/** A dark vignette (edges darker), drawn once. */
function Vignette({ w, h, color, strength = 0.85 }: { w: number; h: number; color: string; strength?: number }) {
  const id = `vig${color.replace('#', '')}`;
  return (
    <View pointerEvents="none" style={StyleSheet.absoluteFill}>
      <Svg width={w} height={h}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="45%" r="75%">
            <Stop offset="0.45" stopColor={color} stopOpacity={0} />
            <Stop offset="1" stopColor={color} stopOpacity={strength} />
          </RadialGradient>
        </Defs>
        <Rect x={0} y={0} width={w} height={h} fill={`url(#${id})`} />
      </Svg>
    </View>
  );
}

// ——— 666: the screen cracks, embers rise, everything pulses red ———

type Segment = { x: number; y: number; len: number; deg: number; at: number; dur: number; thick: number };

/**
 * Broken glass: straight-ish radial cracks from (x, y), then a few concentric rings joining neighbouring cracks.
 * Each segment is drawn after the previous one on its line.
 */
function cracks(r: Rand, x: number, y: number, at: number, w: number, h: number): Segment[] {
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
  // The web: each ring links a crack to the next one with a kinked line.
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

function CrackLine({ clock, s }: { clock: Clock; s: Segment }) {
  const { x, y, deg, at, dur } = s;
  const len = s.len + s.thick;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - at) / dur);
    // Anchored at its start point: the line grows out of (x, y).
    return {
      opacity: p > 0 ? 1 : 0,
      transform: [{ translateX: x }, { translateY: y }, { rotate: `${deg}deg` }, { translateX: (len * p) / 2 }, { scaleX: Math.max(p, 0.001) }],
    };
  });
  return (
    <Animated.View
      style={[{ position: 'absolute', left: -len / 2, top: -s.thick * 1.5, width: len, height: s.thick * 3, justifyContent: 'center' }, style]}>
      <View style={{ position: 'absolute', left: 0, right: 0, height: s.thick * 3, backgroundColor: '#FF2A1A', opacity: 0.22 }} />
      <View style={{ height: s.thick * 0.7, backgroundColor: '#FFD9C4' }} />
    </Animated.View>
  );
}

const EMBER = ['#FF6A00', '#FF3D00', '#FFB347', '#FF2A2A', '#FFD27A'];
const ember = (r: Rand) => {
  const sz = r.range(3, 7);
  return { shape: 'circle' as const, color: r.pick(EMBER), w: sz, h: sz };
};

/** A red heartbeat over the whole screen once the beast has landed. */
function RedPulse({ clock, from }: { clock: Clock; from: number }) {
  const style = useAnimatedStyle(() => {
    const ms = clock.value - from;
    if (ms < 0) return { opacity: 0 };
    // lub-dub every 900 ms
    const ph = (ms % 900) / 900;
    const beat = Math.exp(-Math.pow((ph - 0.08) / 0.05, 2)) + 0.7 * Math.exp(-Math.pow((ph - 0.26) / 0.05, 2));
    return { opacity: 0.08 + 0.22 * beat };
  });
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: '#B3000C' }, style]} />;
}

const devil: SceneFactory = (m, t) => {
  const slam = 1100;
  return {
    duration: 6000,
    bg: '#120000',
    dim: 0.97,
    impacts: [
      { at: slam, amp: 14 },
      { at: slam + 420, amp: 4 },
    ],
    Render: function Devil({ clock, w, h, rand, cx, cy }: SceneProps) {
      const segs = useMemo(() => cracks(rand, cx, cy, slam, w, h), [rand, cx, cy, w, h]);
      const list = useMemo<Particle[]>(
        () => [
          // Embers drifting up from below the whole time, faster after the slam.
          ...Array.from({ length: 56 }, () => ({
            ...ember(rand),
            x: rand.range(0, w),
            y: h + 10,
            vx: rand.range(-20, 20),
            vy: rand.range(-260, -120),
            g: -60,
            k: 0.5,
            sway: rand.range(8, 26),
            swayHz: rand.range(0.4, 1.1),
            at: rand.range(0, 4600),
            life: rand.range(2600, 3800),
            fadeIn: 300,
            fadeOut: 900,
            grow: [1, 0.35] as [number, number],
          })),
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 30,
            speed: [250, 700],
            g: -120,
            k: 1.6,
            life: [1400, 2400],
            fadeOut: 900,
            look: ember,
            grow: [1.2, 0.3],
          }),
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 14,
            speed: [500, 900],
            g: 400,
            k: 2.4,
            life: [500, 800],
            look: sparks(['#FFE0C0', '#FF6A00'], 2.5),
          }),
        ],
        [rand, w, h, cx, cy],
      );
      const size = numberSize(m.n) + 8;
      return (
        <>
          <Glow clock={clock} at={0} x={cx} y={h + 60} color="#FF3D00" size={w * 1.6} opacity={0.35} />
          <RedPulse clock={clock} from={slam + 300} />
          {segs.map((s, i) => (
            <CrackLine key={i} clock={clock} s={s} />
          ))}
          <Vignette w={w} h={h} color="#000000" strength={0.9} />
          <Glow clock={clock} at={slam + 300} x={cx} y={cy + 150} color="#0A0000" size={460} opacity={0.9} />
          <Glow clock={clock} at={slam} x={cx} y={cy} color="#FF1A1A" size={360} opacity={0.55} pulse={1.1} />
          <Flash clock={clock} at={slam} color="#FF2A1A" peak={0.7} fall={520} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FF4A2A" size={620} width={10} dur={750} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 450} cy={cy} box={size * 1.1} ink="#FFE3E0" sub="#E8A49C">
            <Slam clock={clock} at={slam} from={4} fall={260}>
              <BigNumber value={m.n} size={size} color="#FF2A2A" font={t.font.display} glow="#FF0000" />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

// ——— 777: a slot machine that lands on 7-7-7, then a coin fountain ———

type Sym = 'seven' | 'cherry' | 'bell' | 'bar' | 'diamond';
const CELL_W = 84;
const CELL_H = 104;

function SlotSymbol({ kind, font }: { kind: Sym; font: string }) {
  switch (kind) {
    case 'seven':
      return (
        <Text
          style={{
            fontFamily: font,
            fontSize: 84,
            lineHeight: CELL_H,
            color: '#FF2D55',
            includeFontPadding: false,
            textShadowColor: '#7A0018',
            textShadowOffset: { width: 3, height: 4 },
            textShadowRadius: 0,
          }}>
          7
        </Text>
      );
    case 'cherry':
      return (
        <Svg width={60} height={60} viewBox="0 0 60 60">
          <Path d="M20 40 C 24 22, 32 12, 44 8 M40 42 C 40 26, 42 16, 44 8" stroke="#2E8B3A" strokeWidth={3.5} fill="none" strokeLinecap="round" />
          <Path d="M44 8 C 50 4, 56 8, 54 14 C 50 14, 46 12, 44 8 Z" fill="#3DBA4E" />
          <Circle cx={19} cy={43} r={12} fill="#E0102F" />
          <Circle cx={40} cy={44} r={12} fill="#C70A28" />
          <Circle cx={15} cy={39} r={3.5} fill="#FFFFFF" opacity={0.7} />
          <Circle cx={36} cy={40} r={3.5} fill="#FFFFFF" opacity={0.7} />
        </Svg>
      );
    case 'bell':
      return (
        <Svg width={60} height={60} viewBox="0 0 60 60">
          <Path
            d="M30 6 C 18 6, 14 18, 14 30 C 14 38, 10 42, 7 46 L 53 46 C 50 42, 46 38, 46 30 C 46 18, 42 6, 30 6 Z"
            fill="#FFC21A"
            stroke="#B37400"
            strokeWidth={2.5}
          />
          <Circle cx={30} cy={51} r={5} fill="#B37400" />
          <Path d="M22 16 C 19 20, 18 26, 18 32" stroke="#FFF1B8" strokeWidth={3} fill="none" strokeLinecap="round" />
        </Svg>
      );
    case 'diamond':
      return (
        <Svg width={56} height={56} viewBox="0 0 56 56">
          <Polygon points="28,50 4,20 14,8 42,8 52,20" fill="#3DD6F5" stroke="#0A7FA8" strokeWidth={2.5} />
          <Polygon points="4,20 52,20 28,50" fill="#1FB3DB" />
          <Polygon points="14,8 22,20 28,8 34,20 42,8" fill="#9BEFFF" />
        </Svg>
      );
    case 'bar':
      return (
        <View
          style={{
            width: 66,
            height: 34,
            borderRadius: 6,
            backgroundColor: '#15121C',
            borderWidth: 2.5,
            borderColor: '#FFC21A',
            alignItems: 'center',
            justifyContent: 'center',
          }}>
          <Text style={{ fontFamily: font, fontSize: 20, lineHeight: 24, color: '#FFFFFF', letterSpacing: 1, includeFontPadding: false }}>BAR</Text>
        </View>
      );
  }
}

/** One reel: a strip of symbols scrolling up, landing on a 7 (with a small bounce past it). */
function SlotReel({ clock, strip, start, land, font }: { clock: Clock; strip: Sym[]; start: number; land: number; font: string }) {
  // The 7 is second to last: the bounce overshoots onto the one after it.
  const end = (strip.length - 2) * CELL_H;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - start) / (land - start));
    return { transform: [{ translateY: -backOut(p, 1.1) * end }] };
  });
  return (
    <View style={{ width: CELL_W, height: CELL_H, overflow: 'hidden', borderRadius: 10, backgroundColor: '#FFF8E7' }}>
      <Animated.View style={style}>
        {strip.map((k, i) => (
          <View key={i} style={{ width: CELL_W, height: CELL_H, alignItems: 'center', justifyContent: 'center' }}>
            <SlotSymbol kind={k} font={font} />
          </View>
        ))}
      </Animated.View>
      {/* Cylinder shading. */}
      <View pointerEvents="none" style={StyleSheet.absoluteFill}>
        <Svg width={CELL_W} height={CELL_H}>
          <Defs>
            <LinearGradient id="reelshade" x1="0" y1="0" x2="0" y2="1">
              <Stop offset="0" stopColor="#2A145C" stopOpacity={0.75} />
              <Stop offset="0.3" stopColor="#2A145C" stopOpacity={0} />
              <Stop offset="0.7" stopColor="#2A145C" stopOpacity={0} />
              <Stop offset="1" stopColor="#2A145C" stopOpacity={0.75} />
            </LinearGradient>
          </Defs>
          <Rect x={0} y={0} width={CELL_W} height={CELL_H} fill="url(#reelshade)" />
        </Svg>
      </View>
    </View>
  );
}

/** A marquee bulb: chases slowly while the reels spin, then flashes fast on the jackpot. */
function Bulb({ clock, x, y, parity, jackpot }: { clock: Clock; x: number; y: number; parity: number; jackpot: number }) {
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const step = c < jackpot ? Math.floor(c / 220) : Math.floor(c / 110);
    const on = (step + parity) % 2 === 0;
    return { opacity: on ? 1 : 0.25 };
  });
  return (
    <Animated.View
      style={[{ position: 'absolute', left: x - 4, top: y - 4, width: 8, height: 8, borderRadius: 4, backgroundColor: '#FFF1B8' }, glowShadow, style]}
    />
  );
}
const glowShadow: ViewStyle = Platform.OS === 'web' ? ({ boxShadow: '0 0 8px #FFD54A' } as ViewStyle) : {};

const COIN = (
  <Svg width={24} height={24} viewBox="0 0 24 24">
    <Circle cx={12} cy={12} r={11.5} fill="#C98A0B" />
    <Circle cx={12} cy={12} r={9.5} fill="#FFD54A" />
    <Circle cx={12} cy={12} r={6} fill="none" stroke="#E8A317" strokeWidth={1.6} />
    <Path d="M8 7 A 6 6 0 0 1 14 5.5" stroke="#FFF6D0" strokeWidth={1.6} fill="none" strokeLinecap="round" />
  </Svg>
);
const coin = (r: Rand) => ({
  shape: 'node' as const,
  node: COIN,
  w: 24,
  h: 24,
  color: '#FFD54A',
  flip: r.range(7, 15),
  rot: r.range(-30, 30),
  spin: r.range(-90, 90),
});

const jackpot: SceneFactory = (m, t) => {
  const start = 250;
  const lands = [1500, 2050, 2650];
  const win = lands[2];
  return {
    duration: 6200,
    bg: '#120A2A',
    dim: 0.97,
    impacts: [
      { at: lands[0], amp: 3 },
      { at: lands[1], amp: 3 },
      { at: win, amp: 8 },
      { at: win + 120, amp: 4 },
    ],
    Render: function Jackpot({ clock, w, h, rand, cx, cy }: SceneProps) {
      const strips = useMemo(() => {
        const others: Sym[] = ['cherry', 'bell', 'bar', 'diamond', 'cherry', 'bell', 'bar', 'seven'];
        return lands.map((_, i) => [...Array.from({ length: 12 + i * 5 }, () => rand.pick(others)), 'seven' as Sym, rand.pick(others)]);
      }, [rand]);
      const machineW = 3 * CELL_W + 2 * 8 + 2 * 16;
      const machineH = CELL_H + 2 * 16;
      const bulbs = useMemo(() => {
        const out: { x: number; y: number }[] = [];
        const nx = 9;
        for (let i = 0; i < nx; i++) {
          const x = 10 + (i * (machineW - 20)) / (nx - 1);
          out.push({ x, y: 7 }, { x, y: machineH - 7 });
        }
        return out;
      }, [machineW, machineH]);
      const list = useMemo<Particle[]>(
        () => [
          ...burst(rand, {
            x: cx,
            y: cy + machineH / 2,
            at: win + 40,
            count: 34,
            dir: -90,
            spread: 70,
            speed: [900, 1500],
            g: 1500,
            k: 0.4,
            life: [2200, 2800],
            fadeOut: 300,
            look: coin,
          }),
          ...burst(rand, {
            x: cx,
            y: cy + machineH / 2,
            at: win + 650,
            count: 16,
            dir: -90,
            spread: 50,
            speed: [800, 1250],
            g: 1500,
            k: 0.4,
            life: [2000, 2400],
            fadeOut: 300,
            look: coin,
          }),
          ...burst(rand, { x: cx, y: cy, at: win, count: 18, speed: [500, 900], g: 300, k: 2.4, life: [600, 900], look: sparks(GOLD, 3) }),
          ...rain(rand, { w, h, count: 28, from: win + 200, to: win + 2400, look: paper([...GOLD, '#FF2D55', '#B48CFF']), life: 2800 }),
        ],
        [rand, w, h, cx, cy, machineH],
      );
      const font = t.font.display;
      return (
        <>
          <Rays clock={clock} at={win} x={cx} y={cy} color="#FFD54A" size={760} count={16} opacity={0.3} speed={24} />
          <Glow clock={clock} at={win} x={cx} y={cy} color="#FFB300" size={460} opacity={0.45} pulse={1.4} />
          <Flash clock={clock} at={win} color="#FFE082" peak={0.6} fall={500} />
          <Shockwave clock={clock} at={win} x={cx} y={cy} color="#FFE082" size={620} width={8} />
          <Headline clock={clock} m={m} t={t} at={win + 300} cy={cy} box={machineH + 12} ink="#FFFFFF" sub="#C9BEEA">
            <Slam
              clock={clock}
              at={win + 120}
              from={2.4}
              fall={160}
              style={{ position: 'absolute', top: -62, left: 0, right: 0, alignItems: 'center' }}>
              <Text
                style={{
                  fontFamily: font,
                  fontSize: 40,
                  lineHeight: 46,
                  letterSpacing: 1,
                  color: '#FFD54A',
                  includeFontPadding: false,
                  ...glowText('#FFB300', 18),
                }}>
                JACKPOT
              </Text>
            </Slam>
            <Pop clock={clock} at={0}>
              <Keyed clock={clock} times={[win, win + 90, win + 220, win + 360]} k={{ s: [1, 1.09, 0.98, 1] }}>
                <View
                  style={{
                    width: machineW,
                    height: machineH,
                    borderRadius: 22,
                    backgroundColor: '#2A145C',
                    borderWidth: 3,
                    borderColor: '#FFD54A',
                    flexDirection: 'row',
                    alignItems: 'center',
                    justifyContent: 'center',
                    gap: 8,
                  }}>
                  {strips.map((s, i) => (
                    <SlotReel key={i} clock={clock} strip={s} start={start + i * 60} land={lands[i]} font={font} />
                  ))}
                  {bulbs.map((b, i) => (
                    <Bulb key={i} clock={clock} x={b.x - 3} y={b.y - 3} parity={(Math.floor(i / 2) + (i % 2)) % 2} jackpot={win} />
                  ))}
                  {/* The pay line. */}
                  <View
                    pointerEvents="none"
                    style={{
                      position: 'absolute',
                      left: 4,
                      top: machineH / 2 - 9,
                      width: 0,
                      height: 0,
                      borderTopWidth: 6,
                      borderBottomWidth: 6,
                      borderLeftWidth: 9,
                      borderTopColor: 'transparent',
                      borderBottomColor: 'transparent',
                      borderLeftColor: '#FF2D55',
                    }}
                  />
                  <View
                    pointerEvents="none"
                    style={{
                      position: 'absolute',
                      right: 4,
                      top: machineH / 2 - 9,
                      width: 0,
                      height: 0,
                      borderTopWidth: 6,
                      borderBottomWidth: 6,
                      borderRightWidth: 9,
                      borderTopColor: 'transparent',
                      borderBottomColor: 'transparent',
                      borderRightColor: '#FF2D55',
                    }}
                  />
                </View>
              </Keyed>
            </Pop>
          </Headline>
          <Particles clock={clock} list={list} />
        </>
      );
    },
  };
};

// ——— 1000 / 2000: gold fireworks, then MILLE slams down ———

const gold: SceneFactory = (m, t) => {
  const slam = 1450;
  const two = m.n === 2000;
  return {
    duration: 6400,
    bg: '#120C03',
    dim: 0.97,
    impacts: [
      { at: slam, amp: 13 },
      { at: slam + 650, amp: 3 },
    ],
    Render: function Mille({ clock, w, h, rand, cx, cy }: SceneProps) {
      const fw = useMemo(
        () =>
          [
            { x: w * 0.24, y: h * 0.2, at: 0, count: 24 },
            { x: w * 0.76, y: h * 0.15, at: 230, count: 24 },
            { x: w * 0.5, y: h * 0.09, at: 460, count: 24 },
          ].map((s) => firework(rand, { ...s, from: h + 10, colors: GOLD, power: 1.2 })),
        [rand, w, h],
      );
      const list = useMemo<Particle[]>(
        () => [
          ...fw.flatMap((f) => f.list),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 12, speed: [500, 1000], g: 200, k: 2.4, life: [600, 1000], look: sparks(GOLD, 3) }),
          ...rain(rand, { w, h, count: 30, from: slam + 100, to: 4200, look: paper(GOLD), life: 2800 }),
        ],
        [fw, rand, w, h, cx, cy],
      );
      const word = Math.min(98, Math.floor(w / 4));
      const small = 54;
      const box = (two ? word * 0.62 : 0) + word * 1.04 + small * 1.05 + 10;
      const goldText = { color: '#FFD54A', ...glowText('#FFB300', 28) };
      return (
        <>
          {fw.slice(0, 3).map((f, i) => (
            <Flash key={i} clock={clock} at={f.boom} color="#FFD54A" peak={0.12} fall={300} />
          ))}
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#FFD54A" size={820} count={18} opacity={0.32} speed={12} />
          <Glow clock={clock} at={slam} x={cx} y={cy} color="#FFB300" size={440} opacity={0.4} pulse={0.7} />
          <Flash clock={clock} at={slam} color="#FFF1B8" peak={0.55} fall={600} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FFE082" size={560} width={10} dur={650} />
          <Shockwave clock={clock} at={slam + 130} x={cx} y={cy} color="#FFB300" size={820} width={5} dur={900} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 900} cy={cy} box={box} ink="#FFF6D8" sub="#D9C99A">
            <Slam clock={clock} at={slam} from={3.6} fall={240} style={{ alignItems: 'center' }}>
              {two && (
                <Text
                  style={{
                    fontFamily: t.font.display,
                    fontSize: word * 0.6,
                    lineHeight: word * 0.62,
                    letterSpacing: -1,
                    includeFontPadding: false,
                    ...goldText,
                  }}>
                  DEUX
                </Text>
              )}
              <Text
                style={{
                  fontFamily: t.font.display,
                  fontSize: word,
                  lineHeight: word * 1.04,
                  letterSpacing: -2,
                  includeFontPadding: false,
                  ...goldText,
                }}>
                MILLE
              </Text>
            </Slam>
            <Reveal clock={clock} at={slam + 450} rise={-10} style={{ marginTop: 10 }}>
              <RollNumber clock={clock} value={m.n} at={slam + 450} size={small} color="#FFF1B8" font={t.font.display} dur={700} stagger={90} />
            </Reveal>
          </Headline>
        </>
      );
    },
  };
};

// ——— 1337: digital rain, the number decodes itself ———

const HEX = '0123456789ABCDEF';

function RainColumn({
  clock,
  x,
  y0,
  speed,
  text,
  head,
  h,
  opacity,
  size,
}: {
  clock: Clock;
  x: number;
  y0: number;
  speed: number;
  text: string;
  head: string;
  h: number;
  opacity: number;
  size: number;
}) {
  const lineH = Math.round(size * 1.2);
  const colH = (text.length / 2 + 1) * lineH;
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const y = ((y0 + (speed * c) / 1000) % (h + colH)) - colH;
    return { opacity: clamp01(c / 500) * opacity, transform: [{ translateY: y }] };
  });
  return (
    <Animated.View style={[{ position: 'absolute', left: x, top: 0 }, style]}>
      <Text style={{ fontFamily: MONO, fontSize: size, lineHeight: lineH, color: '#22C55E', textAlign: 'center', includeFontPadding: false }}>
        {text}
        <Text style={{ color: '#D8FFE0', ...glowText('#3DFF6E', 8) }}>{head}</Text>
      </Text>
    </Animated.View>
  );
}

/** One character of the number: scrambles through hex digits, stepping (no smooth roll), then locks. */
function DecodeChar({ clock, strip, start, land, size, w }: { clock: Clock; strip: string[]; start: number; land: number; size: number; w: number }) {
  const lineH = Math.round(size * 1.08);
  const n = strip.length;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - start) / (land - start));
    return { opacity: clock.value >= start ? 1 : 0, transform: [{ translateY: -Math.floor(p * (n - 1)) * lineH }] };
  });
  const lock = useAnimatedStyle(() => ({ opacity: interpolate(clock.value, [land, land + 60, land + 400], [0, 1, 0], Extrapolation.CLAMP) }));
  return (
    <View style={{ width: w, height: lineH, overflow: 'hidden' }}>
      <Animated.View style={style}>
        {strip.map((ch, i) => (
          <Text
            key={i}
            style={{
              height: lineH,
              fontFamily: MONO,
              fontWeight: '800',
              fontSize: size,
              lineHeight: lineH,
              textAlign: 'center',
              color: i === n - 1 ? '#C9FFD6' : '#3DFF6E',
              includeFontPadding: false,
            }}>
            {ch}
          </Text>
        ))}
      </Animated.View>
      <Animated.View
        pointerEvents="none"
        style={[StyleSheet.absoluteFill, { backgroundColor: '#3DFF6E' }, lock, { mixBlendMode: 'screen' } as ViewStyle]}
      />
    </View>
  );
}

/** A soft band sweeping down the screen, like an old monitor refreshing. */
function ScanBand({ clock, w, h }: { clock: Clock; w: number; h: number }) {
  const style = useAnimatedStyle(() => ({ transform: [{ translateY: ((clock.value / 2600) % 1) * (h + 160) - 160 }] }));
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: 0, top: 0, width: w, height: 160 }, style]}>
      <Svg width={w} height={160}>
        <Defs>
          <LinearGradient id="scanband" x1="0" y1="0" x2="0" y2="1">
            <Stop offset="0" stopColor="#3DFF6E" stopOpacity={0} />
            <Stop offset="0.5" stopColor="#3DFF6E" stopOpacity={0.07} />
            <Stop offset="1" stopColor="#3DFF6E" stopOpacity={0} />
          </LinearGradient>
        </Defs>
        <Rect x={0} y={0} width={w} height={160} fill="url(#scanband)" />
      </Svg>
    </Animated.View>
  );
}

function Flicker({ clock }: { clock: Clock }) {
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const n = Math.sin(c * 0.091) * Math.sin(c * 0.037 + 1.3);
    return { opacity: 0.05 + 0.06 * Math.max(0, n) };
  });
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: '#000000' }, style]} />;
}

const matrix: SceneFactory = (m, t) => {
  const start = 500;
  const lands = [1300, 1550, 1800, 2050];
  const done = lands[3];
  return {
    duration: 6000,
    bg: '#000000',
    dim: 0.98,
    impacts: [...lands.slice(0, 3).map((at) => ({ at, amp: 1.5 })), { at: done, amp: 6 }],
    Render: function Matrix({ clock, w, h, rand, cx, cy }: SceneProps) {
      const glyph = 15;
      const cols = useMemo(() => {
        const step = 19;
        return Array.from({ length: Math.ceil(w / step) }, (_, i) => {
          const len = Math.floor(rand.range(14, 30));
          const chars = Array.from({ length: len }, () => rand.pick([...HEX, ...'01']));
          return {
            x: i * step + 2,
            y0: rand.range(0, h),
            speed: rand.range(110, 300),
            text: chars.join('\n') + '\n',
            head: rand.pick([...HEX]),
            opacity: rand.range(0.35, 0.85),
          };
        });
      }, [rand, w, h]);
      const size = 92;
      const charW = Math.round(size * 0.64);
      const strips = useMemo(
        () =>
          String(m.n)
            .split('')
            .map((ch, i) => [...Array.from({ length: 14 + i * 4 }, () => rand.pick([...HEX])), ch]),
        [rand],
      );
      const list = useMemo<Particle[]>(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: done,
            count: 34,
            speed: [250, 750],
            g: 600,
            k: 1.8,
            life: [1000, 1600],
            fadeOut: 500,
            look: (r) => ({
              shape: 'text',
              glyph: r.pick([...HEX]),
              font: MONO,
              color: r.pick(['#3DFF6E', '#22C55E', '#C9FFD6']),
              w: 16,
              h: 18,
              spin: r.range(-180, 180),
            }),
          }),
        [rand, cx, cy],
      );
      const box = size * 1.08 + 30;
      return (
        <>
          {cols.map((c, i) => (
            <RainColumn key={i} clock={clock} h={h} size={glyph} {...c} />
          ))}
          <Glow clock={clock} at={0} x={cx} y={cy + 30} color="#000000" size={680} opacity={1} />
          <Glow clock={clock} at={0} x={cx} y={cy} color="#000000" size={420} opacity={1} />
          <Glow clock={clock} at={start} x={cx} y={cy} color="#3DFF6E" size={300} opacity={0.12} />
          <Glow clock={clock} at={done} x={cx} y={cy} color="#3DFF6E" size={380} opacity={0.22} pulse={0.6} />
          <Flash clock={clock} at={done} color="#3DFF6E" peak={0.3} fall={380} />
          <Shockwave clock={clock} at={done} x={cx} y={cy} color="#3DFF6E" size={520} width={4} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={done + 300} cy={cy} box={box} ink="#E6FFEC" sub="#7FD99A">
            <Reveal clock={clock} at={done + 80} rise={6} dur={200}>
              <Text
                style={{
                  fontFamily: MONO,
                  fontSize: 13,
                  lineHeight: 18,
                  letterSpacing: 4,
                  color: '#3DFF6E',
                  marginBottom: 10,
                  ...glowText('#3DFF6E', 8),
                }}>
                ACCÈS AUTORISÉ
              </Text>
            </Reveal>
            <View style={{ flexDirection: 'row' }}>
              {strips.map((s, i) => (
                <DecodeChar key={i} clock={clock} strip={s} start={start} land={lands[i]} size={size} w={charW} />
              ))}
            </View>
          </Headline>
          <ScanBand clock={clock} w={w} h={h} />
          <View pointerEvents="none" style={StyleSheet.absoluteFill}>
            <Svg width={w} height={h}>
              <Defs>
                <Pattern id="scanlines" x={0} y={0} width={4} height={4} patternUnits="userSpaceOnUse">
                  <Rect x={0} y={0} width={4} height={1.5} fill="#000000" opacity={0.35} />
                </Pattern>
              </Defs>
              <Rect x={0} y={0} width={w} height={h} fill="url(#scanlines)" />
            </Svg>
          </View>
          <Flicker clock={clock} />
        </>
      );
    },
  };
};

// ——— 1515 & 1789: bleu, blanc, rouge ———

const TRICOLORE = ['#2F6BFF', '#FFFFFF', '#FF4757'] as const;
const NAVY = '#0B1026';

/** A sword drawn pointing up, `len` px long. */
function Sword({ len }: { len: number }) {
  const w = len * 0.2;
  return (
    <Svg width={w} height={len} viewBox="0 0 44 220">
      <Defs>
        <LinearGradient id="blade" x1="0" y1="0" x2="1" y2="0">
          <Stop offset="0" stopColor="#F4F7FF" />
          <Stop offset="0.5" stopColor="#B8C2D8" />
          <Stop offset="1" stopColor="#7E8AA6" />
        </LinearGradient>
      </Defs>
      <Polygon points="22,0 28,14 28,150 16,150 16,14" fill="url(#blade)" />
      <Path d="M22 8 L22 146" stroke="#8D98B4" strokeWidth={1.2} />
      <Rect x={2} y={148} width={40} height={9} rx={4} fill="#FFC21A" stroke="#B37400" strokeWidth={1.5} />
      <Rect x={17.5} y={157} width={9} height={44} rx={3} fill="#6B3A1E" />
      <G stroke="#3E2010" strokeWidth={1}>
        <Path d="M17.5 166 L26.5 170 M17.5 176 L26.5 180 M17.5 186 L26.5 190" />
      </G>
      <Circle cx={22} cy={208} r={8} fill="#FFC21A" stroke="#B37400" strokeWidth={1.5} />
    </Svg>
  );
}

const marignan: SceneFactory = (m, t) => {
  const clash = 1000;
  const slam = 1500;
  return {
    duration: 5800,
    bg: NAVY,
    dim: 0.97,
    impacts: [
      { at: clash, amp: 10 },
      { at: slam, amp: 5 },
    ],
    Render: function Marignan({ clock, w, h, rand, cx, cy }: SceneProps) {
      const len = 210;
      const sy = cy - 190;
      const list = useMemo<Particle[]>(
        () => [
          ...burst(rand, {
            x: cx,
            y: sy,
            at: clash,
            count: 30,
            speed: [450, 1100],
            g: 700,
            k: 2.2,
            life: [500, 900],
            look: sparks(['#FFFFFF', '#FFE082', '#FFC21A'], 3),
          }),
          ...burst(rand, {
            x: cx,
            y: sy,
            at: clash + 30,
            count: 12,
            speed: [100, 300],
            g: 200,
            k: 2,
            life: [800, 1200],
            look: (r) => ({ shape: 'circle', color: r.pick(['#FFFFFF', '#FFE082']), w: 4, h: 4 }),
          }),
          ...burst(rand, {
            x: -10,
            y: h + 10,
            at: slam,
            count: 30,
            dir: -62,
            spread: 26,
            speed: [1100, 1700],
            g: 950,
            k: 1.3,
            life: [2200, 2800],
            look: paper(TRICOLORE),
          }),
          ...burst(rand, {
            x: w + 10,
            y: h + 10,
            at: slam + 60,
            count: 30,
            dir: -118,
            spread: 26,
            speed: [1100, 1700],
            g: 950,
            k: 1.3,
            life: [2200, 2800],
            look: paper(TRICOLORE),
          }),
          ...rain(rand, { w, h, count: 26, from: slam + 600, to: 3800, look: paper(TRICOLORE), life: 2600 }),
        ],
        [rand, w, h, cx, sy],
      );
      // Each sword swings in from its side, strikes, recoils, and settles crossed.
      const times = [0, clash - 380, clash, clash + 90, clash + 300, clash + 520];
      const size = numberSize(m.n);
      const sword = (side: -1 | 1) => (
        <Keyed
          clock={clock}
          times={times}
          k={{
            x: [side * w * 0.75, side * w * 0.75, 0, side * 14, side * -3, 0],
            y: [80, 80, 0, -6, 2, 0],
            rot: [side * 110, side * 110, side * 38, side * 50, side * 36, side * 40],
            o: [0, 0, 1, 1, 1, 1],
          }}
          style={{ position: 'absolute', left: cx - len * 0.1, top: sy - len * 0.5 }}>
          <Sword len={len} />
        </Keyed>
      );
      return (
        <>
          <Glow clock={clock} at={clash} x={cx} y={sy} color="#FFE082" size={260} opacity={0.6} />
          <Rays clock={clock} at={clash} x={cx} y={sy} color="#FFFFFF" size={520} count={12} opacity={0.14} speed={10} />
          <Flash clock={clock} at={clash} color="#FFFFFF" peak={0.55} fall={300} />
          <Shockwave clock={clock} at={clash} x={cx} y={sy} color="#FFFFFF" size={420} width={6} dur={550} />
          {sword(-1)}
          {sword(1)}
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 300} cy={cy + 40} box={size * 1.1} ink="#FFFFFF" sub="#B9C2E0">
            <Slam clock={clock} at={slam} from={2.6}>
              <BigNumber value={m.n} size={size} color="#FFFFFF" font={t.font.display} glow="#2F6BFF" />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

/** One vertical stripe of the flag, dropping in and then waving gently. */
function Stripe({
  clock,
  x,
  w,
  h,
  color,
  at,
  from,
  phase,
  opacity,
}: {
  clock: Clock;
  x: number;
  w: number;
  h: number;
  color: string;
  at: number;
  from: number;
  phase: number;
  opacity: number;
}) {
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - at) / 650);
    const wave = Math.sin(clock.value / 380 + phase) * 6 * p;
    return { opacity: opacity * Math.min(1, p * 3), transform: [{ translateY: (1 - backOut(p, 0.9)) * from + wave }] };
  });
  return (
    <Animated.View
      pointerEvents="none"
      style={[{ position: 'absolute', left: x, top: -20, width: w, height: h + 40, backgroundColor: color }, style]}
    />
  );
}

/** Light and shade drifting across the stripes, like cloth in the wind. */
function Ripple({ clock, w, h }: { clock: Clock; w: number; h: number }) {
  const style = useAnimatedStyle(() => ({ transform: [{ translateX: -((clock.value / 3200) % 1) * w }] }));
  const stops = Array.from({ length: 9 }, (_, i) => i / 8);
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: 0, top: 0, width: w * 2, height: h }, style]}>
      <Svg width={w * 2} height={h}>
        <Defs>
          <LinearGradient id="ripple" x1="0" y1="0" x2="1" y2="0.18">
            {stops.map((o, i) => (
              <Stop key={i} offset={o} stopColor={i % 2 ? '#000000' : '#FFFFFF'} stopOpacity={i % 2 ? 0.3 : 0.1} />
            ))}
          </LinearGradient>
        </Defs>
        <Rect x={0} y={0} width={w * 2} height={h} fill="url(#ripple)" />
      </Svg>
    </Animated.View>
  );
}

const bastille: SceneFactory = (m, t) => {
  const slam = 1350;
  return {
    duration: 6000,
    bg: NAVY,
    dim: 0.97,
    impacts: [
      { at: slam, amp: 9 },
      { at: 2900, amp: 2 },
    ],
    Render: function Bastille({ clock, w, h, rand, cx, cy }: SceneProps) {
      const fw = useMemo(
        () =>
          [
            { x: w * 0.25, y: h * 0.16, at: 0, colors: ['#2F6BFF', '#9DB8FF'] },
            { x: w * 0.5, y: h * 0.1, at: 220, colors: ['#FFFFFF', '#E6ECFF'] },
            { x: w * 0.75, y: h * 0.16, at: 440, colors: ['#FF4757', '#FFB3BA'] },
            { x: w * 0.5, y: h * 0.14, at: 2400, colors: TRICOLORE },
          ].map((s) => firework(rand, { ...s, from: h + 10, count: 15 })),
        [rand, w, h],
      );
      const list = useMemo<Particle[]>(
        () => [
          ...fw.flatMap((f) => f.list),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 20, speed: [400, 900], g: 900, k: 1.8, life: [1400, 2000], look: paper(TRICOLORE) }),
          ...rain(rand, { w, h, count: 22, from: slam + 300, to: 3800, look: paper(TRICOLORE), life: 2600 }),
        ],
        [fw, rand, w, h, cx, cy],
      );
      const third = w / 3;
      const size = numberSize(m.n);
      return (
        <>
          <Stripe clock={clock} x={0} w={third + 1} h={h} color="#0055A4" at={150} from={-h} phase={0} opacity={0.5} />
          <Stripe clock={clock} x={third} w={third + 1} h={h} color="#FFFFFF" at={350} from={h} phase={0.9} opacity={0.14} />
          <Stripe clock={clock} x={third * 2} w={third + 1} h={h} color="#EF4135" at={550} from={-h} phase={1.8} opacity={0.45} />
          <Ripple clock={clock} w={w} h={h} />
          <Vignette w={w} h={h} color={NAVY} strength={0.9} />
          {fw.slice(0, 3).map((f, i) => (
            <Flash key={i} clock={clock} at={f.boom} color={TRICOLORE[i]} peak={0.08} fall={300} />
          ))}
          <Glow clock={clock} at={slam} x={cx} y={cy} color={NAVY} size={420} opacity={0.85} />
          <Flash clock={clock} at={slam} color="#FFFFFF" peak={0.5} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FFFFFF" size={560} width={8} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 300} cy={cy} box={size * 1.1} ink="#FFFFFF" sub="#D5DBF0">
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color="#FFFFFF" font={t.font.display} glow="#0B1026" />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

export const SCENES: Record<number, SceneFactory> = {
  666: devil,
  777: jackpot,
  1000: gold,
  2000: gold,
  1337: matrix,
  1515: marignan,
  1789: bastille,
};
