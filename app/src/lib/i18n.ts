import { getLocales } from 'expo-localization';
import { useSyncExternalStore } from 'react';
import { Platform } from 'react-native';

import { en } from '@/locales/en';
import { fr, type Messages } from '@/locales/fr';

import { api } from './api';
import { storage } from './storage';

/** Languages the app speaks. French is the source of truth (`locales/fr`), see docs/I18N.md. */
export type Lang = 'fr' | 'en';
/** The Profile setting: follow the phone, or a fixed language. */
export type LangPref = 'auto' | Lang;

/** A string that depends on a count: `{ one: '{count} vote', other: '{count} votes' }`. */
export type Plural = { zero?: string; one: string; other: string };
type Params = Record<string, string | number>;

/** Every key of `locales/fr`, dotted: `'common.cancel'`, `'vote.votes'`. */
type Leaves<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string ? `${P}${K}` : T[K] extends Plural ? `${P}${K}` : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];
export type TKey = Leaves<Messages>;

const TABLES: Record<Lang, Messages> = { fr, en };
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

/** Sends my language to the server when it changed since the last time (account-wide: push, e-mails, share cards). */
export async function syncLang() {
  const state = `${lang}:${pref}`;
  if ((await storage.get(SYNCED_KEY).catch(() => null)) === state) return;
  try {
    await api('/me/locale', { method: 'PUT', json: { locale: lang, pref } });
    await storage.set(SYNCED_KEY, state);
  } catch {} // old server or offline: retried next start
}

/** A fresh sign-in on a device that never chose: take the account's setting, if it has one. */
export async function adoptAccountLang() {
  if (prefSaved) return syncLang();
  try {
    const acc = await api<{ locale: Lang | null; pref: LangPref | null }>('/me/locale');
    if (acc?.pref) return setLangPref(acc.pref, acc.pref === 'auto');
  } catch {}
  return syncLang();
}

export const getLang = () => lang;
export const getLangPref = () => pref;

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

function lookup(table: Messages, key: string): unknown {
  let node: unknown = table;
  for (const part of key.split('.')) node = (node as Record<string, unknown> | undefined)?.[part];
  return node;
}

const fill = (s: string, params?: Params) => (params ? s.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m)) : s);

/** French counts 0 and 1 as singular, English only 1. */
function plural(p: Plural, n: number, l: Lang) {
  if (n === 0 && p.zero != null) return p.zero;
  return (l === 'fr' ? n < 2 : n === 1) ? p.one : p.other;
}

/**
 * The text for `key` in the current language, `{name}` placeholders filled from `params`.
 * Plural keys need `count` (also usable as `{count}`). Falls back to French, then to the key.
 */
export function t(key: TKey, params?: Params): string {
  let v = lookup(TABLES[lang], key);
  if (v == null) v = lookup(fr, key);
  if (typeof v === 'string') return fill(v, params);
  if (v && typeof v === 'object' && 'other' in v) return fill(plural(v as Plural, Number(params?.count ?? 0), lang), params);
  return key;
}

/** `t`, re-rendering the component when the language changes. Use it in every component that shows text. */
export function useT() {
  useSyncExternalStore(subscribe, () => lang);
  return t;
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
