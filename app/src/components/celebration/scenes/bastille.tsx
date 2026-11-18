import { useMemo } from 'react';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Defs, LinearGradient, Rect, Stop } from 'react-native-svg';

import { backOut, clamp01 } from '@/lib/utils/easing';

import { burst, type Clock, firework, paper, type Particle, Particles, rain } from '@/components/celebration/engine';
import { Flash, Glow, Shockwave, Vignette } from '@/components/celebration/effects';
import { NAVY, TRICOLORE } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

const STRIPE_DROP_OVERSHOOT = 0.9;

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
    return { opacity: opacity * Math.min(1, p * 3), transform: [{ translateY: (1 - backOut(p, STRIPE_DROP_OVERSHOOT)) * from + wave }] };
  });
  return (
    <Animated.View
      pointerEvents="none"
      style={[{ position: 'absolute', left: x, top: -20, width: w, height: h + 40, backgroundColor: color }, style]}
    />
  );
}

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

export const bastille: SceneFactory = (m, t) => {
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
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 300} cy={cy} size={size} color="#FFFFFF" glow="#0B1026" ink="#FFFFFF" sub="#D5DBF0" />
        </>
      );
    },
  };
};
