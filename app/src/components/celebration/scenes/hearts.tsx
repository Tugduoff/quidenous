/** 214: hearts — a big heart beats behind the number, small ones float up. */
import { type ReactNode, useMemo } from 'react';
import type { ViewStyle } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Path } from 'react-native-svg';

import { burst, type Clock, type Particle, Particles } from '@/components/celebration/engine';
import { Glow, Shockwave } from '@/components/celebration/effects';
import { centred } from '@/components/celebration/layout';
import { Keyed, Pop } from '@/components/celebration/motion';
import { BigNumber } from '@/components/celebration/numbers';
import { isDark } from '@/components/celebration/palettes';
import { Headline, numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

const PINKS = ['#FF4FA3', '#FF6DB5', '#E8338A', '#FF8FC7', '#FF3B5C'];

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

export const hearts: SceneFactory = (m, t) => {
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
            node: <Heart size={s} color={rand.pick(PINKS)} />,
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
            return { shape: 'node', w: s, h: s * 0.9, rot: r.range(-25, 25), node: <Heart size={s} color={r.pick(PINKS)} /> };
          },
        });
        return [...rising, ...flutter];
      }, [rand, w, h, cx, cy]);
      const size = numberSize(m.n);
      const ink = t.tag;
      return (
        <>
          <Glow clock={clock} at={pop} x={cx} y={cy} color="#FF4FA3" size={420} opacity={isDark(t) ? 0.4 : 0.3} pulse={1} />
          <Keyed clock={clock} style={centred(cx, cy + 6, 330, 300)} k={{ t: [pop, pop + 250], s: [0.5, 1], o: [0, 1] }}>
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
