import { useSyncExternalStore } from 'react';
import { useColorScheme } from 'react-native';

import { storage } from './storage';

const shared = {
  acc: '#C8FF3D',
  accTxt: '#121216',
  radius: { card: 20, pill: 999 },
  font: {
    display: 'BricolageGrotesque_800ExtraBold',
    regular: 'Inter_400Regular',
    semibold: 'Inter_600SemiBold',
    bold: 'Inter_700Bold',
  },
};

export const themes = {
  dark: {
    ...shared,
    bg: '#0E0E12',
    card: '#1A1A21',
    card2: '#24242D',
    line: '#2E2E38',
    txt: '#F5F5F7',
    mut: '#8D8D99',
    accInk: '#C8FF3D',
    accSoft: '#1F2412',
    tag: '#FF4FA3',
    shadow: {},
  },
  light: {
    ...shared,
    bg: '#F5F5F0',
    card: '#FFFFFF',
    card2: '#ECECE5',
    line: '#E2E2DA',
    txt: '#121216',
    mut: '#6E6E78',
    accInk: '#4F7D00',
    accSoft: '#F1FFD0',
    tag: '#E8338A',
    shadow: {
      shadowColor: '#000',
      shadowOpacity: 0.06,
      shadowRadius: 10,
      shadowOffset: { width: 0, height: 2 },
      elevation: 2,
    },
  },
};

export type Theme = typeof themes.dark;

export type ThemePref = 'system' | 'light' | 'dark';

const PREF_KEY = 'qdn.theme';
let pref: ThemePref = 'system';
const listeners = new Set<() => void>();

storage.get(PREF_KEY).then((saved) => {
  if (saved === 'light' || saved === 'dark') setThemePref(saved);
});

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

/** Accent colour, independent of light/dark: buttons, highlights, selections. */
export const ACCENTS = {
  lime: { label: 'Citron', acc: '#C8FF3D', inkDark: '#C8FF3D', inkLight: '#4F7D00', softDark: '#1F2412', softLight: '#F1FFD0' },
  rose: { label: 'Rose', acc: '#FF6DB5', inkDark: '#FF6DB5', inkLight: '#C2186B', softDark: '#2A1220', softLight: '#FFE3F0' },
  bleu: { label: 'Bleu', acc: '#5AC8FA', inkDark: '#5AC8FA', inkLight: '#0A6FA8', softDark: '#0F2230', softLight: '#DFF4FF' },
  orange: { label: 'Orange', acc: '#FFB547', inkDark: '#FFB547', inkLight: '#B26A00', softDark: '#2A1E0B', softLight: '#FFF0D6' },
  violet: { label: 'Violet', acc: '#B48CFF', inkDark: '#C2A3FF', inkLight: '#6A3FD0', softDark: '#1E1830', softLight: '#EFE7FF' },
  menthe: { label: 'Menthe', acc: '#5EF2B0', inkDark: '#5EF2B0', inkLight: '#0F8A5A', softDark: '#0F2A20', softLight: '#DDFBEE' },
} as const;
export type Accent = keyof typeof ACCENTS;

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
