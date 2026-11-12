import { useRef } from 'react';
import { Easing, FadeIn, FadeInDown, FadeOut, LinearTransition, ReduceMotion, ZoomIn } from 'react-native-reanimated';

// Shared motion: short and quiet. Every preset follows the system's reduced-motion setting
// (Reanimated's default, spelled out so it stays true if a preset gets copied around).
const sys = ReduceMotion.System;

/** For things that move like objects (thumbs, presses): settles fast, barely overshoots. */
export const SPRING = { damping: 18, stiffness: 260, mass: 0.6, reduceMotion: sys } as const;
export const EASE = Easing.out(Easing.cubic);

export const motion = {
  /** Siblings sliding into their new place (delete, shuffle, reorder). */
  layout: LinearTransition.duration(220).easing(EASE).reduceMotion(sys),
  enter: FadeIn.duration(200).reduceMotion(sys),
  exit: FadeOut.duration(150).reduceMotion(sys),
  /** A new message: rises a few pixels while fading in. */
  rise: FadeInDown.duration(220).easing(EASE).reduceMotion(sys),
  /** A voter's face joining a stack. */
  pop: ZoomIn.springify().damping(14).stiffness(220).reduceMotion(sys),
  /** The n-th card of a list shown for the first time; the delay stops growing after a few rows. */
  stagger: (i: number) =>
    FadeInDown.duration(240)
      .delay(Math.min(i, 8) * 40)
      .easing(EASE)
      .reduceMotion(sys),
};

/** Ids present when the list first loaded: those appear as they are, only later ones animate in. */
export function useArrivals<K>(ids: K[] | undefined) {
  const first = useRef<Set<K> | null>(null);
  if (ids && !first.current) first.current = new Set(ids);
  return (id: K) => !!first.current && !first.current.has(id);
}
