import { useEffect, useState } from 'react';
import { Easing, useSharedValue, withDelay, withSequence, withSpring, withTiming } from 'react-native-reanimated';
import { claimSeasonSplash } from '@/lib/device/prefs';
import { type ActiveSkin, type SeasonId, seasonTag } from '@/lib/utils/season';

const COLD_START_MS = 10_000;
const BOOTED_AT = Date.now();
let decided = false;

export function useSeasonSplash(skin: ActiveSkin | null): SeasonId | null {
  const [shown, setShown] = useState<SeasonId | null>(null);
  useEffect(() => {
    if (!skin || decided) return;
    decided = true;
    if (Date.now() - BOOTED_AT > COLD_START_MS) return;
    claimSeasonSplash(seasonTag(skin, new Date()))
      .then((first) => {
        if (first) setShown(skin.id);
      })
      .catch(() => {});
  }, [skin]);
  return shown;
}

const SHOW_MS = 1250;
const FADE_IN_MS = { full: 200, reduced: 120 };
const FADE_OUT_MS = { full: 350, reduced: 200 };
const UNMOUNT_SLACK_MS = 20;
const SKIP_FADE_MS = 150;
const SKIP_MS = 160;
const SETTLE_DELAY_MS = 60;
const SETTLE_SPRING = { damping: 13, stiffness: 150, mass: 0.7 };

export function useSplashTimeline(reduced: boolean) {
  const [leaving, setLeaving] = useState(false);
  const [skipped, setSkipped] = useState(false);
  const [gone, setGone] = useState(false);
  const show = useSharedValue(0);
  const settle = useSharedValue(reduced ? 1 : 0);

  useEffect(() => {
    const motion = reduced ? 'reduced' : 'full';
    show.set(
      withSequence(
        withTiming(1, { duration: FADE_IN_MS[motion], easing: Easing.out(Easing.cubic) }),
        withDelay(SHOW_MS - FADE_IN_MS[motion], withTiming(0, { duration: FADE_OUT_MS[motion], easing: Easing.in(Easing.quad) })),
      ),
    );
    if (!reduced) settle.set(withDelay(SETTLE_DELAY_MS, withSpring(1, SETTLE_SPRING)));
    const fade = setTimeout(() => setLeaving(true), SHOW_MS);
    const done = setTimeout(() => setGone(true), SHOW_MS + FADE_OUT_MS.full + UNMOUNT_SLACK_MS);
    return () => {
      clearTimeout(fade);
      clearTimeout(done);
    };
  }, [reduced, show, settle]);

  useEffect(() => {
    if (!skipped) return;
    const done = setTimeout(() => setGone(true), SKIP_MS);
    return () => clearTimeout(done);
  }, [skipped]);

  const skip = () => {
    if (leaving) return;
    setLeaving(true);
    setSkipped(true);
    show.set(withTiming(0, { duration: SKIP_FADE_MS }));
  };

  return { show, settle, leaving, gone, skip };
}
