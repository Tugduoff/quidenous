import { useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';

import { clamp01 } from '@/lib/utils/easing';
import { cracks, type Segment } from '@/lib/utils/cracks';
import type { Rand } from '@/lib/utils/rand';

import { burst, type Clock, type Particle, Particles, sparks } from '@/components/celebration/engine';
import { Flash, Glow, Shockwave, Vignette } from '@/components/celebration/effects';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function CrackLine({ clock, s }: { clock: Clock; s: Segment }) {
  const { x, y, deg, at, dur } = s;
  const len = s.len + s.thick;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - at) / dur);
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

function RedPulse({ clock, from }: { clock: Clock; from: number }) {
  const style = useAnimatedStyle(() => {
    const ms = clock.value - from;
    if (ms < 0) return { opacity: 0 };
    const ph = (ms % 900) / 900;
    const beat = Math.exp(-Math.pow((ph - 0.08) / 0.05, 2)) + 0.7 * Math.exp(-Math.pow((ph - 0.26) / 0.05, 2));
    return { opacity: 0.08 + 0.22 * beat };
  });
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: '#B3000C' }, style]} />;
}

export const devil: SceneFactory = (m, t) => {
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
          <SlamNumber
            clock={clock}
            m={m}
            t={t}
            slam={slam}
            at={slam + 450}
            cy={cy}
            size={size}
            color="#FF2A2A"
            glow="#FF0000"
            ink="#FFE3E0"
            sub="#E8A49C"
            from={4}
            fall={260}
          />
        </>
      );
    },
  };
};
