import { useEffect } from 'react';
import { Platform } from 'react-native';
import { cancelAnimation, Easing, type SharedValue, useSharedValue, withTiming } from 'react-native-reanimated';
import { impactHaptic } from '@/lib/device/haptics';
import { useMountEffect } from '@/lib/hooks/use-mount-effect';

export function useCelebrationClock(duration: number, frozen?: number | null): SharedValue<number> {
  const c = useSharedValue(frozen ?? 0);
  useEffect(() => {
    if (frozen != null) return;
    c.value = withTiming(duration, { duration, easing: Easing.linear });
    return () => cancelAnimation(c);
  }, [c, duration, frozen]);
  return c;
}

export function useCelebrationTimers({
  frozen,
  reduced,
  duration,
  impacts,
  onEnd,
}: {
  frozen: number | null | undefined;
  reduced: boolean;
  duration: number;
  impacts: readonly { at: number; amp: number }[];
  onEnd: () => void;
}) {
  useMountEffect(() => {
    if (frozen != null) return;
    const timers = [setTimeout(onEnd, reduced ? 4500 : duration)];
    if (!reduced && Platform.OS !== 'web')
      for (const i of impacts) timers.push(setTimeout(() => impactHaptic(i.amp >= 8 ? 'heavy' : i.amp >= 4 ? 'medium' : 'light'), i.at));
    return () => timers.forEach(clearTimeout);
  });
}
