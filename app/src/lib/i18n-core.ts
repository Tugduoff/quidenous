// The translation tables and lookup, with no app or native dependency: shared by lib/i18n.ts (the app) and the
// home-screen widget's drawing (widget/widget.tsx, also rendered off-device by scripts/widget-preview.mjs). See docs/I18N.md.
import { en } from '@/locales/en';
import { fr, type Messages } from '@/locales/fr';

/** Languages the app speaks. French is the source of truth (`locales/fr`), see docs/I18N.md. */
export type Lang = 'fr' | 'en';

/** A string that depends on a count: `{ one: '{count} vote', other: '{count} votes' }`. */
type Plural = { zero?: string; one: string; other: string };
export type Params = Record<string, string | number>;

/** Every key of `locales/fr`, dotted: `'common.cancel'`, `'vote.votes'`. */
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

/** French counts 0 and 1 as singular, English only 1. */
function plural(p: Plural, n: number, l: Lang) {
  if (n === 0 && p.zero != null) return p.zero;
  return (l === 'fr' ? n < 2 : n === 1) ? p.one : p.other;
}

/**
 * French typography for display: a no-break space before ? ! : ; » and after «, so the mark never wraps onto its own line.
 * Any run of spaces (or an existing no-break space) collapses to one. `t` applies it to every French string.
 */
export const tidy = (s: string) => s.replace(/\s+([?!:;»])/g, '\u00a0$1').replace(/«\s+/g, '«\u00a0');

/** `t` in a given language, whatever the app shows (the home-screen widget speaks the account's language). */
export function tIn(l: Lang, key: TKey, params?: Params): string {
  let v = lookup(TABLES[l], key);
  if (v == null) v = lookup(fr, key);
  const s = typeof v === 'string' ? v : v && typeof v === 'object' && 'other' in v ? plural(v as Plural, Number(params?.count ?? 0), l) : null;
  if (s == null) return key;
  return l === 'fr' ? tidy(fill(s, params)) : fill(s, params);
}
