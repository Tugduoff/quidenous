import { requireOptionalNativeModule } from 'expo';
import { useSyncExternalStore } from 'react';
import { Platform } from 'react-native';

import { storage } from './storage';

type LocalAuth = typeof import('expo-local-authentication');

// Loaded lazily: a build without the native module (older binary getting a JS update) and the web just have no app lock.
let LA: LocalAuth | null = null;
if (Platform.OS !== 'web' && requireOptionalNativeModule('ExpoLocalAuthentication')) {
  try {
    LA = require('expo-local-authentication') as LocalAuth;
  } catch {}
}

export const lockSupported = LA != null;

const KEY = 'qdn.lock';
let enabled = false;
let loaded = !lockSupported;
const listeners = new Set<() => void>();
const notify = () => listeners.forEach((l) => l());

if (lockSupported)
  storage
    .get(KEY)
    .then((v) => {
      enabled = v === '1';
    })
    .catch(() => {})
    .finally(() => {
      loaded = true;
      notify();
    });

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

/** Whether the app lock is on, and whether the saved choice is known yet (the app waits for it at start). */
export function useLock() {
  return {
    enabled: useSyncExternalStore(subscribe, () => enabled),
    loaded: useSyncExternalStore(subscribe, () => loaded),
  };
}

/** Face ID / fingerprint, or the device passcode. */
export async function unlock(): Promise<boolean> {
  if (!LA) return true;
  const r = await LA.authenticateAsync({ promptMessage: 'Déverrouiller qui de nous ?', cancelLabel: 'Annuler' });
  return r.success;
}

/** Turning it on asks once, so a phone with no passcode (or a failed try) never locks itself out. */
export async function setLock(on: boolean): Promise<'ok' | 'unavailable' | 'failed'> {
  if (!LA) return 'unavailable';
  if (on) {
    if ((await LA.getEnrolledLevelAsync()) === LA.SecurityLevel.NONE) return 'unavailable';
    if (!(await unlock())) return 'failed';
  }
  enabled = on;
  await storage.set(KEY, on ? '1' : '0').catch(() => {});
  notify();
  return 'ok';
}
