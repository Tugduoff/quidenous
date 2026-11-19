import { getLocales } from 'expo-localization';
import { useSyncExternalStore } from 'react';
import { Platform } from 'react-native';

import { type Lang, type Params, type TKey, tIn } from '@/lib/i18n-core';
import { storage } from '@/lib/device/storage';
import { setMyLocale, getMyLocale } from '@/lib/api/me';

export { type Lang, type TKey, tidy } from '@/lib/i18n-core';
export type LangPref = 'auto' | Lang;

const PREF_KEY = 'qdn.lang';
const SYNCED_KEY = 'qdn.lang.synced';

export function deviceLang(): Lang {
  const code = getLocales()[0]?.languageCode;
  return !code || code === 'fr' ? 'fr' : 'en';
}

let pref: LangPref = 'auto';
let lang: Lang = deviceLang();
let prefSaved = false;
const listeners = new Set<() => void>();

function apply(next: LangPref) {
  pref = next;
  lang = next === 'auto' ? deviceLang() : next;
  formatters.clear();
  if (Platform.OS === 'web' && typeof document !== 'undefined') document.documentElement.lang = lang;
  listeners.forEach((l) => l());
}

export async function loadLang(signedIn: boolean) {
  const saved = (await storage.get(PREF_KEY).catch(() => null)) as LangPref | null;
  if (saved === 'auto' || saved === 'fr' || saved === 'en') {
    prefSaved = true;
    apply(saved);
  } else if (signedIn) {
    await setLangPref('fr', false);
  } else apply('auto');
}

export async function setLangPref(next: LangPref, sync = true) {
  prefSaved = true;
  apply(next);
  await storage.set(PREF_KEY, next).catch(() => {});
  if (sync) await syncLang();
}

export const showLang = (next: Lang) => next !== lang && apply(next);

export async function syncLang() {
  const state = `${lang}:${pref}`;
  if ((await storage.get(SYNCED_KEY).catch(() => null)) === state) return;
  try {
    await setMyLocale(lang, pref);
    await storage.set(SYNCED_KEY, state);
  } catch {}
}

export async function adoptAccountLang() {
  if (prefSaved) return syncLang();
  try {
    const acc = await getMyLocale();
    if (acc?.pref) return setLangPref(acc.pref, acc.pref === 'auto');
  } catch {}
  return syncLang();
}

export const getLang = () => lang;

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

export function t(key: TKey, params?: Params): string {
  return tIn(lang, key, params);
}

const translators: Partial<Record<Lang, typeof t>> = {};

export function useT(): typeof t {
  const l = useSyncExternalStore(subscribe, () => lang);
  return (translators[l] ??= (key, params) => t(key, params));
}

export function useLang(): { lang: Lang; pref: LangPref } {
  const l = useSyncExternalStore(subscribe, () => lang);
  const p = useSyncExternalStore(subscribe, () => pref);
  return { lang: l, pref: p };
}

const intlLocale = () => (lang === 'fr' ? 'fr-FR' : 'en-GB');

const formatters = new Map<string, Intl.DateTimeFormat | Intl.NumberFormat>();

export function dateFormat(opts: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const k = `d${JSON.stringify(opts)}`;
  let f = formatters.get(k);
  if (!f) formatters.set(k, (f = new Intl.DateTimeFormat(intlLocale(), opts)));
  return f as Intl.DateTimeFormat;
}

export function formatNumber(n: number, opts?: Intl.NumberFormatOptions): string {
  const k = `n${JSON.stringify(opts ?? {})}`;
  let f = formatters.get(k);
  if (!f) formatters.set(k, (f = new Intl.NumberFormat(intlLocale(), opts)));
  return (f as Intl.NumberFormat).format(n);
}
