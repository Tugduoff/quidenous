import { useEffect, useEffectEvent } from 'react';

type Key = string | number | boolean | null | undefined;

export function useTimeout(callback: () => void, ms: number | null, restartKey?: Key) {
  const fire = useEffectEvent(callback);
  useEffect(() => {
    if (ms == null) return;
    const id = setTimeout(fire, ms);
    return () => clearTimeout(id);
  }, [ms, restartKey]);
}
