import { useEffect, useEffectEvent } from 'react';
import { AppState, type AppStateStatus } from 'react-native';

export function useAppStateChange(onChange: (state: AppStateStatus) => void, enabled = true) {
  const handle = useEffectEvent(onChange);
  useEffect(() => {
    if (!enabled) return;
    const sub = AppState.addEventListener('change', (state) => handle(state));
    return () => sub.remove();
  }, [enabled]);
}
