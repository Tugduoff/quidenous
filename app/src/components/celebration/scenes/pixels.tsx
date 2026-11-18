/** 256, 512: pixels — an old console's falling squares and a hard hit flash. */
import { type ReactNode, useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';

import type { Clock } from '@/components/celebration/engine';
import { Scanlines } from '@/components/celebration/effects';
import { BigNumber } from '@/components/celebration/numbers';
import { PIXEL } from '@/components/celebration/palettes';
import { Headline, numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

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

export const pixels: SceneFactory = (m, t) => {
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
      const size = numberSize(m.n);
      return (
        <>
          <Scanlines w={w} h={h} color="#FFFFFF" opacity={0.035} thick={1} />
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
