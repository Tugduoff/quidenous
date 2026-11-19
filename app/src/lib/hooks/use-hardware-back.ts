import { useEffect, useEffectEvent } from 'react';
import { BackHandler, Platform } from 'react-native';

export function useHardwareBack(onBack: () => void, enabled: boolean) {
  const handle = useEffectEvent(onBack);
  useEffect(() => {
    if (!enabled || Platform.OS !== 'android') return;
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      handle();
      return true;
    });
    return () => sub.remove();
  }, [enabled]);
}
