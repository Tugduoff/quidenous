import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useEffectEvent, useState } from 'react';
import { Easing, ReduceMotion, type SharedValue, useSharedValue, withTiming } from 'react-native-reanimated';
import { stepAt } from '@/lib/utils/party';
import { setApiRoom } from '@/lib/api/client';
import { type Lang, showLang } from '@/lib/i18n';
import { useSession } from '@/lib/session';

export function useEnterPartyRoom(id: number, loaded: boolean) {
  const qc = useQueryClient();
  const { room, setRoom } = useSession();
  useEffect(() => {
    if (!loaded || room === id) return;
    setApiRoom(id);
    qc.removeQueries({ predicate: (q) => q.queryKey[0] !== 'party' });
    setRoom(id);
  }, [loaded, room, id, qc, setRoom]);
}

export function useGuestScreenLanguage(language: Lang | undefined, signedIn: boolean) {
  useEffect(() => {
    if (language && !signedIn) showLang(language);
  }, [language, signedIn]);
}

/**
 * Where the big screen's reveal is: the step (an answer) or `steps.length` (the scoreboard), from the time since the
 * question closed on the server's clock. So a screen opened late, or reloaded, catches up, and two screens agree.
 */
export function useScreenStep(closedAt: string | null, clock: number, steps: number[]) {
  const key = steps.join(',');
  const [step, setStep] = useState(() => stepAt(closedAt, clock, steps, Date.now()).step);
  const stepNow = useEffectEvent(() => stepAt(closedAt, clock, steps, Date.now()));
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      const now = stepNow();
      setStep(now.step);
      if (now.left != null) timer = setTimeout(tick, now.left + 30);
    };
    tick();
    return () => clearTimeout(timer);
  }, [closedAt, clock, key]);
  return step;
}

export function useCountdownBar(frac: number, next: number, running: boolean): SharedValue<number> {
  const w = useSharedValue(frac);
  useEffect(() => {
    w.value = frac;
    if (running) w.value = withTiming(next, { duration: 1000, easing: Easing.linear, reduceMotion: ReduceMotion.Never });
  }, [frac, next, running, w]);
  return w;
}
