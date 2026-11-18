import { useEffect } from 'react';
import { cancelAnimation, Easing, makeMutable, ReduceMotion, type SharedValue, useSharedValue, withRepeat, withTiming } from 'react-native-reanimated';

export const MASCOT_LOOP = 120;

export const mascotClock = makeMutable(0);
let users = 0;

function startClock() {
  if (users++ > 0) return;
  mascotClock.value = 0;
  mascotClock.value = withRepeat(withTiming(MASCOT_LOOP, { duration: MASCOT_LOOP * 1000, easing: Easing.linear, reduceMotion: ReduceMotion.Never }), -1, false);
}

function stopClock() {
  if (--users > 0) return;
  cancelAnimation(mascotClock);
}

export function useMascotClock(live: boolean): SharedValue<number> {
  const on = useSharedValue(live ? 1 : 0);
  useEffect(() => {
    on.value = live ? 1 : 0;
    if (!live) return;
    startClock();
    return stopClock;
  }, [live, on]);
  return on;
}
