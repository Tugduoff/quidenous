import { useEffect, useEffectEvent } from 'react';

export function useMountEffect(effect: () => void | (() => void)) {
  const run = useEffectEvent(effect);
  useEffect(() => run(), []);
}

export function useUnmountEffect(cleanup: () => void) {
  const run = useEffectEvent(cleanup);
  useEffect(() => () => run(), []);
}
