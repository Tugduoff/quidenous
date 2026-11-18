import { en } from '@/locales/en';
import { fr, type Messages } from '@/locales/fr';

export type Lang = 'fr' | 'en';

type Plural = { zero?: string; one: string; other: string };
export type Params = Record<string, string | number>;

type Leaves<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string ? `${P}${K}` : T[K] extends Plural ? `${P}${K}` : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];
export type TKey = Leaves<Messages>;

const TABLES: Record<Lang, Messages> = { fr, en };

function lookup(table: Messages, key: string): unknown {
  let node: unknown = table;
  for (const part of key.split('.')) node = (node as Record<string, unknown> | undefined)?.[part];
  return node;
}

const fill = (s: string, params?: Params) => (params ? s.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m)) : s);

function plural(p: Plural, n: number, l: Lang) {
  if (n === 0 && p.zero != null) return p.zero;
  return (l === 'fr' ? n < 2 : n === 1) ? p.one : p.other;
}

export const tidy = (s: string) => s.replace(/\s+([?!:;»])/g, '\u00a0$1').replace(/«\s+/g, '«\u00a0');

export function tIn(l: Lang, key: TKey, params?: Params): string {
  let v = lookup(TABLES[l], key);
  if (v == null) v = lookup(fr, key);
  const s = typeof v === 'string' ? v : v && typeof v === 'object' && 'other' in v ? plural(v as Plural, Number(params?.count ?? 0), l) : null;
  if (s == null) return key;
  return l === 'fr' ? tidy(fill(s, params)) : fill(s, params);
}
