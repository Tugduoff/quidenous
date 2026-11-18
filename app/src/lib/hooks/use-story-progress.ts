import { useEffect, useEffectEvent } from 'react';
import { cancelAnimation, Easing, ReduceMotion, useSharedValue, withTiming } from 'react-native-reanimated';
import { scheduleOnRN } from 'react-native-worklets';

export function useStoryProgress({ index, count, auto, cardMs, onEnd }: { index: number; count: number; auto: boolean; cardMs: number; onEnd: () => void }) {
  const progress = useSharedValue(0);
  const fill = (from: number, then?: () => void) => {
    progress.set(
      withTiming(1, { duration: cardMs * (1 - from), easing: Easing.linear, reduceMotion: ReduceMotion.Never }, (finished) => {
        'worklet';
        if (finished && then) scheduleOnRN(then);
      }),
    );
  };
  const start = useEffectEvent(() => {
    cancelAnimation(progress);
    if (!auto) {
      progress.set(1);
      return;
    }
    progress.set(0);
    fill(0, index < count - 1 ? onEnd : undefined);
  });
  useEffect(() => start(), [index, auto, count]);
  const pause = () => {
    if (auto) cancelAnimation(progress);
  };
  const resume = () => {
    if (!auto) return;
    const at = progress.get();
    if (at >= 1) return;
    fill(at, index < count - 1 ? onEnd : undefined);
  };
  return { progress, pause, resume };
}
