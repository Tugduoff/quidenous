import { useEffect } from 'react';
import { cancelAnimation, Easing, makeMutable, ReduceMotion, withRepeat, withTiming } from 'react-native-reanimated';
import { SNOW_LOOP_S } from '@/lib/utils/snowfall';

export const snowClock = makeMutable(0);
let users = 0;

function startClock() {
  if (users++ > 0) return;
  snowClock.value = 0;
  snowClock.value = withRepeat(withTiming(SNOW_LOOP_S, { duration: SNOW_LOOP_S * 1000, easing: Easing.linear, reduceMotion: ReduceMotion.Never }), -1, false);
}

function stopClock() {
  if (--users > 0) return;
  cancelAnimation(snowClock);
}

export function useSnowClock(live: boolean) {
  useEffect(() => {
    if (!live) return;
    startClock();
    return stopClock;
  }, [live]);
}
