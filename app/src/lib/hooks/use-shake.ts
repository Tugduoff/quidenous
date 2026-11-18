import { useMemo } from 'react';
import { type SharedValue, useAnimatedStyle } from 'react-native-reanimated';

/** Impacts: when they hit (ms) and how hard (px). Shake decays fast; strong ones also buzz the phone. */
export type Impact = { at: number; amp: number };

/** Style for the whole stage: a short, decaying shake after each impact. */
export function useShake(clock: SharedValue<number>, impacts: Impact[]) {
  const list = useMemo(() => impacts.map((i) => [i.at, i.amp]), [impacts]);
  return useAnimatedStyle(() => {
    let x = 0;
    let y = 0;
    for (let i = 0; i < list.length; i++) {
      const dt = (clock.value - list[i][0]) / 1000;
      if (dt < 0 || dt > 0.45) continue;
      const a = list[i][1] * Math.exp(-dt * 11);
      x += a * Math.sin(dt * 95);
      y += a * 0.7 * Math.cos(dt * 120);
    }
    return { transform: [{ translateX: x }, { translateY: y }] };
  });
}
