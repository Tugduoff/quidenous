import { getLocales } from 'expo-localization';
import { useSyncExternalStore } from 'react';
import { Platform } from 'react-native';

import { type Lang, type Params, type TKey, tIn } from '@/lib/i18n-core';
import { storage } from '@/lib/device/storage';
import { setMyLocale, getMyLocale } from '@/lib/api/me';

export { type Lang, type Plural, type TKey, tidy, tIn } from '@/lib/i18n-core';
/** The Profile setting: follow the phone, or a fixed language. */
export type LangPref = 'auto' | Lang;

const PREF_KEY = 'qdn.lang';
const SYNCED_KEY = 'qdn.lang.synced';

/** The phone's language: French stays French, anything else gets English. */
export function deviceLang(): Lang {
  const code = getLocales()[0]?.languageCode;
  return !code || code === 'fr' ? 'fr' : 'en';
}

let pref: LangPref = 'auto';
let lang: Lang = deviceLang();
/** Whether this device has a saved choice (else a fresh sign-in adopts the account's). */
let prefSaved = false;
const listeners = new Set<() => void>();

function apply(next: LangPref) {
  pref = next;
  lang = next === 'auto' ? deviceLang() : next;
  formatters.clear();
  if (Platform.OS === 'web' && typeof document !== 'undefined') document.documentElement.lang = lang;
  listeners.forEach((l) => l());
}

/** Called once at start, before the first screen. An install from before this setting (signed in already) stays French. */
export async function loadLang(signedIn: boolean) {
  const saved = (await storage.get(PREF_KEY).catch(() => null)) as LangPref | null;
  if (saved === 'auto' || saved === 'fr' || saved === 'en') {
    prefSaved = true;
    apply(saved);
  } else if (signedIn) {
    await setLangPref('fr', false);
  } else apply('auto');
}

/** Profile setting. `sync`: tell the server (push and e-mails follow the account's language). */
export async function setLangPref(next: LangPref, sync = true) {
  prefSaved = true;
  apply(next);
  await storage.set(PREF_KEY, next).catch(() => {});
  if (sync) await syncLang();
}

/** Shows the app in `next` without saving it (the party's big screen speaks the party's language, not the TV's). */
export const showLang = (next: Lang) => next !== lang && apply(next);

/** Sends my language to the server when it changed since the last time (account-wide: push, e-mails, share cards). */
export async function syncLang() {
  const state = `${lang}:${pref}`;
  if ((await storage.get(SYNCED_KEY).catch(() => null)) === state) return;
  try {
    await setMyLocale(lang, pref);
    await storage.set(SYNCED_KEY, state);
  } catch {} // old server or offline: retried next start
}

/** A fresh sign-in on a device that never chose: take the account's setting, if it has one. */
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


/**
 * The text for `key` in the current language, `{name}` placeholders filled from `params`.
 * Plural keys need `count` (also usable as `{count}`). Falls back to French, then to the key.
 */
export function t(key: TKey, params?: Params): string {
  return tIn(lang, key, params);
}


/** One `t` per language: a new function when the language changes, so memoised (React Compiler) text is recomputed. */
const translators: Partial<Record<Lang, typeof t>> = {};

/** `t`, re-rendering the component when the language changes. Use it in every component that shows text. */
export function useT(): typeof t {
  const l = useSyncExternalStore(subscribe, () => lang);
  return (translators[l] ??= (key, params) => t(key, params));
}

export function useLang(): { lang: Lang; pref: LangPref } {
  const l = useSyncExternalStore(subscribe, () => lang);
  const p = useSyncExternalStore(subscribe, () => pref);
  return { lang: l, pref: p };
}

/** BCP 47 tag for Intl: dates, times and numbers in the current language. */
export const intlLocale = () => (lang === 'fr' ? 'fr-FR' : 'en-GB');

const formatters = new Map<string, Intl.DateTimeFormat | Intl.NumberFormat>();

/** A cached Intl.DateTimeFormat for the current language (rebuilt when it changes). */
export function dateFormat(opts: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const k = `d${JSON.stringify(opts)}`;
  let f = formatters.get(k);
  if (!f) formatters.set(k, (f = new Intl.DateTimeFormat(intlLocale(), opts)));
  return f as Intl.DateTimeFormat;
}

/** A number in the current language (`1 234,5` / `1,234.5`). */
export function formatNumber(n: number, opts?: Intl.NumberFormatOptions): string {
  const k = `n${JSON.stringify(opts ?? {})}`;
  let f = formatters.get(k);
  if (!f) formatters.set(k, (f = new Intl.NumberFormat(intlLocale(), opts)));
  return (f as Intl.NumberFormat).format(n);
}
