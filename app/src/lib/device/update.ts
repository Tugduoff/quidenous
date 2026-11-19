import * as Application from 'expo-application';
import Constants from 'expo-constants';
import { Directory, File, Paths } from 'expo-file-system';
import * as IntentLauncher from 'expo-intent-launcher';
import { useSyncExternalStore } from 'react';
import { Platform } from 'react-native';
import { storage } from '@/lib/device/storage';
import { latestRelease, type Release } from '@/lib/api/releases';

export const SIDELOAD = Platform.OS === 'android' && process.env.EXPO_PUBLIC_DISTRIBUTION === 'sideload';

const PREVIEW =
  (__DEV__ || process.env.EXPO_PUBLIC_UPDATE_PREVIEW === '1') && Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('maj')
    : null;

export const UPDATES = SIDELOAD || PREVIEW != null;
const CHECKED_KEY = 'qdn.update.checked';
const LATER_KEY = 'qdn.update.later';
const EVERY = 6 * 3600_000;
const LATER_FOR = 24 * 3600_000;
const APK_TYPE = 'application/vnd.android.package-archive';
const FLAG_GRANT_READ_URI_PERMISSION = 1;
type Status = 'idle' | 'checking' | 'latest' | 'failed' | 'download' | 'blocked' | 'error';
type State = { release: Release | null; open: boolean; status: Status; progress: number };

export function appVersion() {
  return Constants.expoConfig?.version ?? (Platform.OS === 'web' ? null : Application.nativeApplicationVersion) ?? '?';
}

function isNewer(a: string, b: string) {
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
  sha256: '',
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
    const release = await latestRelease();
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

export function later() {
  if (state.release && !PREVIEW) storage.set(LATER_KEY, JSON.stringify({ version: state.release.version, at: Date.now() })).catch(() => {});
  set({ open: false, status: state.status === 'download' ? state.status : 'idle' });
}

export function clearCheck() {
  if (state.status === 'latest' || state.status === 'failed') set({ status: 'idle' });
}

export async function install() {
  const release = state.release;
  if (!release || PREVIEW || state.status === 'download') return;
  try {
    const dir = new Directory(Paths.cache, 'updates');
    if (!dir.exists) dir.create({ intermediates: true });
    const apk = new File(dir, `quidenous-${release.version}.apk`);
    if (!apk.exists || apk.size !== release.size) {
      for (const f of dir.list()) f.delete();
      set({ status: 'download', progress: 0 });
      await File.downloadFileAsync(release.url, apk, {
        idempotent: true,
        onProgress: ({ bytesWritten, totalBytes }) => set({ progress: bytesWritten / (totalBytes > 0 ? totalBytes : release.size) }),
      });
    }
    if ((await apk.digest('SHA-256')).toLowerCase() !== release.sha256) {
      apk.delete();
      throw new Error('APK checksum mismatch');
    }
    set({ status: 'idle', progress: 1 });
    await IntentLauncher.startActivityAsync('android.intent.action.VIEW', {
      data: apk.contentUri,
      type: APK_TYPE,
      flags: FLAG_GRANT_READ_URI_PERMISSION,
    });
    set({ status: 'blocked' });
  } catch {
    set({ status: 'error', progress: 0 });
  }
}

export function cleanUp() {
  try {
    const dir = new Directory(Paths.cache, 'updates');
    if (!dir.exists) return;
    for (const f of dir.list()) if (!isNewer(f.name.replace(/^quidenous-|\.apk$/g, ''), appVersion())) f.delete();
  } catch {}
}

export function openInstallSettings() {
  if (PREVIEW) return;
  IntentLauncher.startActivityAsync(IntentLauncher.ActivityAction.MANAGE_UNKNOWN_APP_SOURCES, {
    data: `package:${Application.applicationId}`,
  }).catch(() => {});
}
