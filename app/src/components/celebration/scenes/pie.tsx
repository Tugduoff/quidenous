/** 314: a pie fills like a pie chart, the digits of π spiral around it. */
import { useMemo } from 'react';
import { Text, View } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, Line, RadialGradient, Stop } from 'react-native-svg';

import { clamp01, easeInOutCubic } from '@/lib/utils/easing';

import { burst, type Clock, paper, type Particle, Particles } from '@/components/celebration/engine';
import { Flash, Glow, Shockwave } from '@/components/celebration/effects';
import { ABS } from '@/components/celebration/layout';
import { Orbiter, Pop } from '@/components/celebration/motion';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

const PI = '3,14159265358979323846264338327950';
const BAKERY = ['#F2A541', '#E07A2E', '#FFD9A0', '#C98A45', '#8C4A21'];

export const pie: SceneFactory = (m, t) => {
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
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 300} cy={ny} size={size} color="#FFB547" ink="#FFE9C8" sub="#C9A27E" />
        </>
      );
    },
  };
};

/** The pie: an empty tin, filling clockwise from noon (two half-discs turning behind two clipping halves), then the crust and lattice. */
function PieDish({ clock, R, from, to }: { clock: Clock; R: number; from: number; to: number }) {
  const progress = (c: number) => {
    'worklet';
    return easeInOutCubic(clamp01((c - from) / (to - from)));
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
