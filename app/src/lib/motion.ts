import { useState } from 'react';
import { useOnChange } from '@/lib/hooks/use-on-change';
import {
  Easing,
  FadeIn,
  FadeInDown,
  FadeInUp,
  FadeOut,
  LinearTransition,
  ReduceMotion,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withSequence,
  withSpring,
  withTiming,
  ZoomIn,
} from 'react-native-reanimated';

const sys = ReduceMotion.System;

export const SPRING = { damping: 18, stiffness: 260, mass: 0.6, reduceMotion: sys } as const;
export const EASE = Easing.out(Easing.cubic);
export const LAYOUT_EASE = Easing.bezier(0.33, 1, 0.68, 1);
export const BOUNCE = { damping: 11, stiffness: 300, mass: 0.6, reduceMotion: sys } as const;

export const motion = {
  layout: LinearTransition.duration(220).easing(LAYOUT_EASE).reduceMotion(sys),
  enter: FadeIn.duration(200).reduceMotion(sys),
  exit: FadeOut.duration(150).reduceMotion(sys),
  rise: FadeInDown.duration(220).easing(LAYOUT_EASE).reduceMotion(sys),
  drop: FadeInUp.duration(200).easing(LAYOUT_EASE).reduceMotion(sys),
  pop: ZoomIn.springify().damping(14).stiffness(220).reduceMotion(sys),
  question: FadeInDown.duration(280).easing(LAYOUT_EASE).reduceMotion(sys),
  stagger: (i: number) =>
    FadeInDown.duration(240)
      .delay(Math.min(i, 8) * 40)
      .easing(LAYOUT_EASE)
      .reduceMotion(sys),
};

export function useArrivals<K>(ids: K[] | undefined) {
  const [first, setFirst] = useState<Set<K> | null>(null);
  if (ids && !first) setFirst(new Set(ids));
  return (id: K) => !!first && !first.has(id);
}

export function usePop(trigger: unknown, to = 1.15, when = true) {
  const reduced = useReducedMotion();
  const s = useSharedValue(1);
  useOnChange(trigger, () => {
    if (reduced || !when) return;
    s.set(withSequence(withTiming(to, { duration: 90, easing: EASE }), withSpring(1, BOUNCE)));
  });
  return useAnimatedStyle(() => ({ transform: [{ scale: s.value }] }));
}
