import type { PackTag } from './packs';

/** What a pack file's header says (absent = not said). */
export type PackHeader = { title?: string; description?: string; categories: PackTag[]; lang?: 'fr' | 'en'; spicy?: boolean };

/** Lowercase, accents stripped, trimmed: how keys and values are compared. */
const norm = (s: string) =>
  s
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .trim();

type Key = 'title' | 'description' | 'categories' | 'lang' | 'spicy';

const KEYS = new Map<string, Key>(
  Object.entries({
    titre: 'title',
    title: 'title',
    nom: 'title',
    name: 'title',
    description: 'description',
    desc: 'description',
    categorie: 'categories',
    categories: 'categories',
    category: 'categories',
    tags: 'categories',
    tag: 'categories',
    langue: 'lang',
    lang: 'lang',
    language: 'lang',
    '16+': 'spicy',
  } as const),
);

/** Category ids, French and English names (normalised) → id. Fixed here: a renamed label must not break old files. */
const CATEGORY_NAMES = new Map<string, PackTag>(
  Object.entries({
    potes: 'potes',
    pimente: 'pimente',
    boulot: 'boulot',
    famille: 'famille',
    soiree: 'soiree',
    culture: 'culture',
    absurde: 'absurde',
    'entre potes': 'potes',
    friends: 'potes',
    spicy: 'pimente',
    work: 'boulot',
    family: 'famille',
    party: 'soiree',
    absurd: 'absurde',
  } as const),
);

const LANGS = new Map<string, 'fr' | 'en'>(Object.entries({ fr: 'fr', francais: 'fr', french: 'fr', en: 'en', anglais: 'en', english: 'en' } as const));
const YES = ['oui', 'o', 'yes', 'y', 'true', '1'];
const NO = ['non', 'n', 'no', 'false', '0'];

/**
 * A pack file's header (same rules as the server): the `# key: value` lines at the top, before the first question line
 * (blank lines allowed). Unknown keys, `#` lines without a known key and empty values are comments; a repeated key: the last wins.
 */
export function parsePackHeader(content: string): PackHeader {
  const raw: Partial<Record<Key, string>> = {};
  for (const l of content.replace(/^\uFEFF/, '').split(/\r?\n/)) {
    const line = l.trim();
    if (!line) continue;
    if (!line.startsWith('#')) break;
    const body = line.replace(/^#+/, '');
    const colon = body.indexOf(':');
    if (colon < 0) continue;
    const key = KEYS.get(norm(body.slice(0, colon)));
    const value = body.slice(colon + 1).trim();
    if (key && value) raw[key] = value;
  }

  const head: PackHeader = { categories: [] };
  if (raw.title) head.title = raw.title;
  if (raw.description) head.description = raw.description;
  if (raw.categories) {
    for (const part of raw.categories.split(/[,;]/)) {
      const tag = CATEGORY_NAMES.get(norm(part).replace(/16\+$/, '').trim());
      if (tag && !head.categories.includes(tag)) head.categories.push(tag);
    }
    head.categories = head.categories.slice(0, 3);
  }
  if (raw.lang) {
    const lang = LANGS.get(norm(raw.lang));
    if (lang) head.lang = lang;
  }
  if (raw.spicy) {
    const v = norm(raw.spicy);
    if (YES.includes(v)) head.spicy = true;
    else if (NO.includes(v)) head.spicy = false;
  }
  if (head.categories.includes('pimente')) head.spicy = true;
  return head;
}
