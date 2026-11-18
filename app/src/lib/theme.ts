import { useSyncExternalStore } from 'react';
import { useColorScheme } from 'react-native';

import { storage } from '@/lib/device/storage';
import { ACCENTS, themes, type Accent } from '@/lib/tokens';

export { ACCENTS, type Accent } from '@/lib/tokens';

export type Theme = typeof themes.dark;

export type ThemePref = 'system' | 'light' | 'dark';

const PREF_KEY = 'qdn.theme';
let pref: ThemePref = 'system';
const listeners = new Set<() => void>();

let prefLoaded = false;
storage
  .get(PREF_KEY)
  .then((saved) => {
    if (saved === 'light' || saved === 'dark') pref = saved;
  })
  .catch(() => {})
  .finally(() => {
    prefLoaded = true;
    listeners.forEach((l) => l());
  });

export function useThemeLoaded(): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => prefLoaded,
  );
}

export function setThemePref(next: ThemePref) {
  pref = next;
  storage.set(PREF_KEY, next).catch(() => {});
  listeners.forEach((l) => l());
}

export function useThemePref(): ThemePref {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => pref,
  );
}

export function useScheme(): 'light' | 'dark' {
  const system = useColorScheme();
  const chosen = useThemePref();
  return chosen === 'system' ? (system === 'light' ? 'light' : 'dark') : chosen;
}


const ACCENT_KEY = 'qdn.accent';
let accent: Accent = 'lime';
const accentListeners = new Set<() => void>();
storage.get(ACCENT_KEY).then((saved) => {
  if (saved && saved in ACCENTS) setAccent(saved as Accent);
});

export function setAccent(next: Accent) {
  accent = next;
  storage.set(ACCENT_KEY, next).catch(() => {});
  accentListeners.forEach((l) => l());
}

export function useAccent(): Accent {
  return useSyncExternalStore(
    (l) => {
      accentListeners.add(l);
      return () => accentListeners.delete(l);
    },
    () => accent,
  );
}

const cache = new Map<string, Theme>();

export function useTheme(): Theme {
  const scheme = useScheme();
  const a = useAccent();
  const key = `${scheme}-${a}`;
  let theme = cache.get(key);
  if (!theme) {
    const c = ACCENTS[a];
    const dark = scheme === 'dark';
    theme = { ...themes[scheme], acc: c.acc, accInk: dark ? c.inkDark : c.inkLight, accSoft: dark ? c.softDark : c.softLight };
    cache.set(key, theme);
  }
  return theme;
}
