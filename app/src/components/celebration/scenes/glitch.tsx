/** 404: glitch — the number tears into bands, ghosts and noise. */
import type { ReactNode } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';

import { noise } from '@/lib/utils/easing';

import type { Clock } from '@/components/celebration/engine';
import { Scanlines, SweepBand } from '@/components/celebration/effects';
import { BigNumber } from '@/components/celebration/numbers';
import { Headline, numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

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
    const on = noise(step * 7.13 + i * 1.7) > 1 - g * 0.65;
    const dx = on ? (noise(step * 13.7 + i * 3.1) - 0.5) * 70 * g : 0;
    const flick = c < GLITCH_ON ? 0 : c < 700 ? (noise(step * 3.3) > 0.3 ? 1 : 0.15) : 1;
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
    const dx = dir * (3 + noise(step * 5.1 + dir) * 14 * g);
    const dy = (noise(step * 9.7 - dir) - 0.5) * 8 * g;
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
    const on = noise(step * 31.1 + i * 4.7) < g * 0.55;
    const x = noise(step * 11.3 + i) * w - 60;
    const y = noise(step * 17.9 + i * 2.3) * h;
    const sx = 0.2 + noise(step * 23.1 + i) * 1.6;
    return { opacity: on ? 0.75 : 0, transform: [{ translateX: x }, { translateY: y }, { scaleX: sx }] };
  });
  return (
    <Animated.View style={[{ position: 'absolute', left: 0, top: 0, width: 120, height: 3 + (i % 4) * 4, backgroundColor: colors[i % 3] }, a]} />
  );
}

export const glitch: SceneFactory = (m, t) => ({
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
    const number = (color: string) => <BigNumber value={m.n} size={size} color={color} font={t.font.display} />;
    return (
      <>
        {Array.from({ length: 12 }, (_, i) => (
          <Noise key={i} clock={clock} i={i} w={w} h={h} />
        ))}
        <SweepBand clock={clock} w={w} h={h} period={1700} size={120} reach={160} peak={0.85} color="#FFFFFF" />
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
        <Scanlines w={w} h={h} color="#000000" opacity={0.35} thick={1.2} />
      </>
    );
  },
});
