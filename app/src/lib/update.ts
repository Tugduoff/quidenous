import * as Application from 'expo-application';
import Constants from 'expo-constants';
import { Directory, File, Paths } from 'expo-file-system';
import * as IntentLauncher from 'expo-intent-launcher';
import { useEffect, useSyncExternalStore } from 'react';
import { AppState, Platform } from 'react-native';

import { storage } from './storage';

/**
 * In-app updates for the APK installed by hand (GitHub Releases). Only in builds made with
 * EXPO_PUBLIC_DISTRIBUTION=sideload: the Google Play bundle never self-updates (Play policy).
 */
export const SIDELOAD = Platform.OS === 'android' && process.env.EXPO_PUBLIC_DISTRIBUTION === 'sideload';

/** Screenshots only (`?maj=offer|download|blocked`, in a dev or EXPO_PUBLIC_UPDATE_PREVIEW=1 web build): mocked states. */
const PREVIEW =
  (__DEV__ || process.env.EXPO_PUBLIC_UPDATE_PREVIEW === '1') && Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('maj')
    : null;

/** Updates can be offered here (or mocked for a screenshot). */
export const UPDATES = SIDELOAD || PREVIEW != null;

const LATEST = 'https://api.github.com/repos/Tugduoff/quidenous/releases/latest';
const CHECKED_KEY = 'qdn.update.checked';
const LATER_KEY = 'qdn.update.later';
const EVERY = 6 * 3600_000;
const LATER_FOR = 24 * 3600_000;
const APK_TYPE = 'application/vnd.android.package-archive';
const FLAG_GRANT_READ_URI_PERMISSION = 1;

export type Release = { version: string; notes: string; url: string; size: number };
type Status = 'idle' | 'checking' | 'latest' | 'failed' | 'download' | 'blocked' | 'error';
type State = { release: Release | null; open: boolean; status: Status; progress: number };

/**
 * The version running: the app config of the JS bundle, which an over-the-air update brings along (so a JS-only
 * release that already arrived over the air is not offered again as an APK), else the version of the build.
 */
export function appVersion() {
  return Constants.expoConfig?.version ?? (Platform.OS === 'web' ? null : Application.nativeApplicationVersion) ?? '?';
}

/** `a` is a later version than `b` (x.y.z, a leading "v" ignored). */
export function isNewer(a: string, b: string) {
  const parse = (v: string) =>
    v
      .replace(/^v/, '')
      .split(/[.-]/)
      .slice(0, 3)
      .map((n) => parseInt(n, 10) || 0);
  const [x, y] = [parse(a), parse(b)];
  for (let i = 0; i < 3; i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0);
  return false;
}

const MOCK: Release = {
  version: '0.19.0',
  notes: '- Mise à jour depuis l’app : un appui suffit\n- Version affichée dans le profil\n- Petites corrections',
  url: '',
  size: 71_000_000,
};
const PREVIEW_STATE: Record<string, Partial<State>> = {
  offer: {},
  download: { status: 'download', progress: 0.42 },
  blocked: { status: 'blocked' },
};

let state: State =
  PREVIEW && PREVIEW in PREVIEW_STATE
    ? { release: MOCK, open: true, status: 'idle', progress: 0, ...PREVIEW_STATE[PREVIEW] }
    : { release: null, open: false, status: 'idle', progress: 0 };
const listeners = new Set<() => void>();
const set = (next: Partial<State>) => {
  state = { ...state, ...next };
  listeners.forEach((l) => l());
};

export function useUpdate() {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => state,
  );
}

async function latest(): Promise<Release | null> {
  const res = await fetch(LATEST, { headers: { Accept: 'application/vnd.github+json' } });
  if (!res.ok) throw new Error(`GitHub ${res.status}`);
  const body = (await res.json()) as {
    tag_name: string;
    body?: string | null;
    draft?: boolean;
    prerelease?: boolean;
    assets: { name: string; size: number; browser_download_url: string }[];
  };
  const apk = body.assets.find((a) => a.name.endsWith('.apk'));
  if (!apk || body.draft || body.prerelease) return null;
  return { version: body.tag_name.replace(/^v/, ''), notes: body.body ?? '', url: apk.browser_download_url, size: apk.size };
}

/**
 * Looks for a newer release. Automatic checks run at most every 6 h and respect "Plus tard"
 * (asked again for the next version, or after 24 h); a manual check always shows what it finds.
 */
export async function checkForUpdate(manual = false) {
  if (!UPDATES || state.status === 'checking' || state.status === 'download') return;
  if (PREVIEW) {
    if (manual) set({ open: true });
    return;
  }
  const now = Date.now();
  if (!manual && now - Number((await storage.get(CHECKED_KEY)) ?? 0) < EVERY) return;
  if (manual) set({ status: 'checking' });
  try {
    const release = await latest();
    await storage.set(CHECKED_KEY, String(now));
    if (!release || !isNewer(release.version, appVersion())) {
      set({ release: null, open: false, status: manual ? 'latest' : 'idle' });
      return;
    }
    const later = JSON.parse((await storage.get(LATER_KEY)) ?? 'null') as { version: string; at: number } | null;
    const snoozed = later?.version === release.version && now - later.at < LATER_FOR;
    set({ release, open: manual || !snoozed || state.open, status: 'idle' });
  } catch {
    set({ status: manual ? 'failed' : 'idle' });
  }
}

/** "Plus tard": closes it until the next version, or for 24 h. */
export function later() {
  if (state.release && !PREVIEW) storage.set(LATER_KEY, JSON.stringify({ version: state.release.version, at: Date.now() })).catch(() => {});
  set({ open: false, status: state.status === 'download' ? state.status : 'idle' });
}

/** Back to idle after a manual check's answer has been shown. */
export function clearCheck() {
  if (state.status === 'latest' || state.status === 'failed') set({ status: 'idle' });
}

/** Downloads the APK (kept in the cache, reused on a retry) and hands it to Android's package installer. */
export async function install() {
  const release = state.release;
  if (!release || PREVIEW || state.status === 'download') return;
  try {
    const dir = new Directory(Paths.cache, 'updates');
    if (!dir.exists) dir.create({ intermediates: true });
    const apk = new File(dir, `quidenous-${release.version}.apk`);
    if (!apk.exists || apk.size !== release.size) {
      // Older downloads go: one APK at most in the cache.
      for (const f of dir.list()) f.delete();
      set({ status: 'download', progress: 0 });
      await File.downloadFileAsync(release.url, apk, {
        idempotent: true,
        onProgress: ({ bytesWritten, totalBytes }) => set({ progress: bytesWritten / (totalBytes > 0 ? totalBytes : release.size) }),
      });
    }
    set({ status: 'idle', progress: 1 });
    await IntentLauncher.startActivityAsync('android.intent.action.VIEW', {
      data: apk.contentUri,
      type: APK_TYPE,
      flags: FLAG_GRANT_READ_URI_PERMISSION,
    });
    // Still running: the update was not installed (cancelled, or installs from this app are not allowed yet).
    set({ status: 'blocked' });
  } catch {
    set({ status: 'error', progress: 0 });
  }
}

/** The APK of the version now installed is no longer needed. */
function cleanUp() {
  try {
    const dir = new Directory(Paths.cache, 'updates');
    if (!dir.exists) return;
    for (const f of dir.list()) if (!isNewer(f.name.replace(/^quidenous-|\.apk$/g, ''), appVersion())) f.delete();
  } catch {}
}

/** Android's "Install unknown apps" switch for this app. */
export function openInstallSettings() {
  if (PREVIEW) return;
  IntentLauncher.startActivityAsync(IntentLauncher.ActivityAction.MANAGE_UNKNOWN_APP_SOURCES, {
    data: `package:${Application.applicationId}`,
  }).catch(() => {});
}

/** Checks on start and each time the app comes back to the foreground (at most every 6 h). */
export function useUpdateCheck(active: boolean) {
  useEffect(() => {
    if (!active || !UPDATES) return;
    if (SIDELOAD) cleanUp();
    checkForUpdate();
    const sub = AppState.addEventListener('change', (s) => s === 'active' && checkForUpdate());
    return () => sub.remove();
  }, [active]);
}
