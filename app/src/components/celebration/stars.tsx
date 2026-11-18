/** A starry sky: three layers of stars twinkling out of phase. */
import { useMemo } from 'react';
import { StyleSheet } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle } from 'react-native-svg';

import type { Rand } from '@/lib/utils/rand';

import type { Clock } from '@/components/celebration/engine';

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
export function StarField({
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
