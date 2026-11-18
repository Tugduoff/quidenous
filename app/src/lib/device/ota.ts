import * as Updates from 'expo-updates';
import { useState } from 'react';
import { Platform } from 'react-native';

export const OTA = Platform.OS !== 'web' && Updates.isEnabled;

const PREVIEW =
  (__DEV__ || process.env.EXPO_PUBLIC_UPDATE_PREVIEW === '1') && Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('ota') === 'ready'
    : false;

const EVERY = 30 * 60_000;
let checked = Date.now();

export async function checkOta() {
  if (Date.now() - checked < EVERY) return;
  checked = Date.now();
  try {
    if ((await Updates.checkForUpdateAsync()).isAvailable) await Updates.fetchUpdateAsync();
  } catch {}
}

export function useOtaReady() {
  const { isUpdatePending } = Updates.useUpdates();
  const [dismissed, setDismissed] = useState(false);
  return { ready: (PREVIEW || (OTA && isUpdatePending)) && !dismissed, dismiss: () => setDismissed(true) };
}

export function restart() {
  if (OTA) Updates.reloadAsync().catch(() => {});
}
