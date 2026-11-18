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

// Shared motion: short and quiet. Every preset follows the system's reduced-motion setting
// (Reanimated's default, spelled out so it stays true if a preset gets copied around).
const sys = ReduceMotion.System;

/** For things that move like objects (thumbs, presses): settles fast, barely overshoots. */
export const SPRING = { damping: 18, stiffness: 260, mass: 0.6, reduceMotion: sys } as const;
export const EASE = Easing.out(Easing.cubic);
/** The same curve as a bezier, for entering/layout animations: the web runs those in CSS, which takes no other easing. */
export const LAYOUT_EASE = Easing.bezier(0.33, 1, 0.68, 1);
/** A little livelier, for pops: one small overshoot, settled in about 300 ms. */
export const BOUNCE = { damping: 11, stiffness: 300, mass: 0.6, reduceMotion: sys } as const;

export const motion = {
  /** Siblings sliding into their new place (delete, shuffle, reorder). */
  layout: LinearTransition.duration(220).easing(LAYOUT_EASE).reduceMotion(sys),
  enter: FadeIn.duration(200).reduceMotion(sys),
  exit: FadeOut.duration(150).reduceMotion(sys),
  /** A new message: rises a few pixels while fading in. */
  rise: FadeInDown.duration(220).easing(LAYOUT_EASE).reduceMotion(sys),
  /** Something pinned to the top dropping into place (the result summary). */
  drop: FadeInUp.duration(200).easing(LAYOUT_EASE).reduceMotion(sys),
  /** A voter's face joining a stack. */
  pop: ZoomIn.springify().damping(14).stiffness(220).reduceMotion(sys),
  /** A new question: slides up a little while fading in. (No custom offset: on the web that takes it out of the layout.) */
  question: FadeInDown.duration(280).easing(LAYOUT_EASE).reduceMotion(sys),
  /** The n-th card of a list shown for the first time; the delay stops growing after a few rows. */
  stagger: (i: number) =>
    FadeInDown.duration(240)
      .delay(Math.min(i, 8) * 40)
      .easing(LAYOUT_EASE)
      .reduceMotion(sys),
};

/** Ids present when the list first loaded: those appear as they are, only later ones animate in. */
export function useArrivals<K>(ids: K[] | undefined) {
  const [first, setFirst] = useState<Set<K> | null>(null);
  if (ids && !first) setFirst(new Set(ids));
  return (id: K) => !!first && !first.has(id);
}

/**
 * A quick scale bump (1 → `to` → 1, ~300 ms) each time `trigger` changes, never on mount.
 * `when` gates it (e.g. only when something got selected, not deselected).
 */
export function usePop(trigger: unknown, to = 1.15, when = true) {
  const reduced = useReducedMotion();
  const s = useSharedValue(1);
  useOnChange(trigger, () => {
    if (reduced || !when) return;
    s.set(withSequence(withTiming(to, { duration: 90, easing: EASE }), withSpring(1, BOUNCE)));
  });
  return useAnimatedStyle(() => ({ transform: [{ scale: s.value }] }));
}
