import * as Updates from 'expo-updates';
import { useEffect, useState } from 'react';
import { AppState, Platform } from 'react-native';

/**
 * Over-the-air updates (expo-updates, our own server; docs/OTA.md). The native side checks on each launch and
 * downloads in the background; here, coming back to the foreground checks again (at most every 30 min).
 * A downloaded update applies on the next cold start, or right away with "Relancer".
 */
export const OTA = Platform.OS !== 'web' && Updates.isEnabled;

/** Screenshots only (`?ota=ready`, in a dev or EXPO_PUBLIC_UPDATE_PREVIEW=1 web build). */
const PREVIEW =
  (__DEV__ || process.env.EXPO_PUBLIC_UPDATE_PREVIEW === '1') && Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('ota') === 'ready'
    : false;

const EVERY = 30 * 60_000;
// The launch check is the native one: the first foreground check waits 30 min from here.
let checked = Date.now();

async function check() {
  if (Date.now() - checked < EVERY) return;
  checked = Date.now();
  try {
    if ((await Updates.checkForUpdateAsync()).isAvailable) await Updates.fetchUpdateAsync();
  } catch {
    // Offline, server down or a check already running: the next launch or foreground tries again.
  }
}

/** Foreground checks. */
export function useOtaCheck(active: boolean) {
  useEffect(() => {
    if (!active || !OTA) return;
    const sub = AppState.addEventListener('change', (s) => s === 'active' && check());
    return () => sub.remove();
  }, [active]);
}

/** A new version has been downloaded and waits for a restart. */
export function useOtaReady() {
  const { isUpdatePending } = Updates.useUpdates();
  const [dismissed, setDismissed] = useState(false);
  return { ready: (PREVIEW || (OTA && isUpdatePending)) && !dismissed, dismiss: () => setDismissed(true) };
}

/** Restarts on the downloaded version. */
export function restart() {
  if (OTA) Updates.reloadAsync().catch(() => {});
}
