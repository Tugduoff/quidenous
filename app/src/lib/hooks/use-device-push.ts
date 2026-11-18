import { Platform, AppState } from 'react-native';
import { pushState, webSubscribed, enablePush } from '@/lib/device/web-push';
import { nativePushState, registerNativePush } from '@/lib/device/native-push';
import { useState, useCallback, useEffect } from 'react';

/**
 * This device: 'on' = allowed and subscribed, 'off' = can still be turned on, 'denied' = blocked by the browser / phone,
 * 'unsupported' = no push here (iPhone app, a browser without push).
 */
export type DevicePush = 'unsupported' | 'off' | 'denied' | 'on';

async function devicePush(): Promise<DevicePush> {
  if (Platform.OS === 'web') {
    const s = pushState();
    if (s === 'unsupported' || s === 'denied') return s;
    return s === 'granted' && (await webSubscribed()) ? 'on' : 'off';
  }
  const s = await nativePushState();
  return s === 'granted' ? 'on' : s === 'default' ? 'off' : s;
}

/** The device's state, checked again when the app comes back (from the phone's settings). `enable` must run from a tap. */
export function useDevicePush() {
  const [state, setState] = useState<DevicePush | null>(null);
  const refresh = useCallback(() => devicePush().then(setState, () => setState('unsupported')), []);
  useEffect(() => {
    refresh();
    const sub = AppState.addEventListener('change', (s) => s === 'active' && refresh());
    return () => sub.remove();
  }, [refresh]);
  const enable = async () => {
    try {
      if (Platform.OS === 'web') await enablePush();
      else await registerNativePush();
    } finally {
      await refresh();
    }
  };
  return { state, enable };
}
