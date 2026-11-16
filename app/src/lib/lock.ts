import { requireOptionalNativeModule } from 'expo';
import { useSyncExternalStore } from 'react';
import { Platform } from 'react-native';

import { t } from './i18n';
import { storage } from './storage';

type LocalAuth = typeof import('expo-local-authentication');

// Loaded lazily: a build without the native module (older binary getting a JS update) and the web just have no app lock.
let LA: LocalAuth | null = null;
if (Platform.OS !== 'web' && requireOptionalNativeModule('ExpoLocalAuthentication')) {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    LA = require('expo-local-authentication') as LocalAuth;
  } catch {}
}

export const lockSupported = LA != null;

/** What the phone unlocks with, to name it in the settings: face, fingerprint, or just the passcode. */
export async function lockMethod(): Promise<'face' | 'fingerprint' | 'code'> {
  if (!LA) return 'code';
  try {
    const types = await LA.supportedAuthenticationTypesAsync();
    const enrolled = await LA.isEnrolledAsync();
    if (enrolled && types.includes(LA.AuthenticationType.FACIAL_RECOGNITION)) return 'face';
    if (enrolled && types.includes(LA.AuthenticationType.FINGERPRINT)) return 'fingerprint';
  } catch {}
  return 'code';
}

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

/** The phone has no passcode or biometrics any more (removed after the lock was turned on). */
async function noCredentials(la: LocalAuth): Promise<boolean> {
  try {
    return (await la.getEnrolledLevelAsync()) === la.SecurityLevel.NONE;
  } catch {
    return false;
  }
}

/**
 * Face ID / fingerprint, or the device passcode. With nothing left to unlock with, the lock turns
 * itself off instead of locking the app for good (the phone itself is then open anyway).
 */
export async function unlock(): Promise<boolean> {
  if (!LA) return true;
  if (await noCredentials(LA)) {
    await setLock(false);
    return true;
  }
  const r = await LA.authenticateAsync({ promptMessage: t('lock.prompt'), cancelLabel: t('common.cancel') });
  if (!r.success && (r.error === 'not_enrolled' || r.error === 'passcode_not_set')) {
    await setLock(false);
    return true;
  }
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
