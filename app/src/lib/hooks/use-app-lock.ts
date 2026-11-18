import { useRef, useState } from 'react';
import { AppState } from 'react-native';
import { lockMethod, unlock } from '@/lib/device/lock';
import { useAppStateChange } from '@/lib/hooks/use-app-state';
import { useMountEffect } from '@/lib/hooks/use-mount-effect';
import { useWhen } from '@/lib/hooks/use-on-change';

const AWAY_MS = 5 * 60_000;

export function useAppLock(on: boolean) {
  const [locked, setLocked] = useState(on);
  if (!on && locked) setLocked(false);
  const leftAt = useRef<number | null>(null);
  const asking = useRef(false);

  const ask = async () => {
    if (asking.current) return;
    asking.current = true;
    try {
      if (await unlock()) setLocked(false);
    } finally {
      asking.current = false;
    }
  };

  useAppStateChange((state) => {
    if (state === 'background') leftAt.current = Date.now();
    if (state === 'active' && leftAt.current != null) {
      if (Date.now() - leftAt.current > AWAY_MS) setLocked(true);
      leftAt.current = null;
    }
  }, on);

  useWhen(locked, () => {
    if (AppState.currentState === 'active') ask();
  });

  return { locked: locked && on, ask };
}

type LockMethod = Awaited<ReturnType<typeof lockMethod>>;

export function useLockMethod() {
  const [method, setMethod] = useState<LockMethod>('code');
  useMountEffect(() => {
    lockMethod().then(setMethod);
  });
  return method;
}
