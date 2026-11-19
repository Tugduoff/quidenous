const MIN = 8;
const MAX = 200;

export const ITEMS_MIN = 2;
export const ITEMS_MAX = 4;
export const ITEM_MAX = 40;

type DraftProblem = 'short' | 'long' | 'duplicate' | 'items';

export type DraftKind = 'pick' | 'rank' | 'choose';

export type DraftLine = {
  line: string;
  text: string;
  rank: boolean;
  kind: DraftKind;
  items: string[] | null;
  anonymous: boolean;
  selected: boolean;
  problem: DraftProblem | null;
};

const VIBE_TAGS = ['potes', 'famille', 'boulot', 'pimente'];

export function splitTag(line: string): { text: string; rank: boolean; kind: DraftKind; items: string[] | null; anonymous: boolean } {
  let rest = line.trim();
  let kind: DraftKind = 'pick';
  let anonymous = false;
  for (;;) {
    const m = /^\[([^\]]*)\]/.exec(rest);
    if (!m) break;
    const tag = m[1].trim().toLowerCase();
    if (tag === 'classement' || tag === 'rank') kind = 'rank';
    else if (tag === 'choix' || tag === 'choose') kind = 'choose';
    else if (tag === 'anonyme' || tag === 'anonymous') anonymous = true;
    else if (!VIBE_TAGS.includes(tag)) break;
    rest = rest.slice(m[0].length).trimStart();
  }
  if (kind !== 'choose') return { text: rest, rank: kind === 'rank', kind, items: null, anonymous };
  const [text, ...parts] = rest.split('|');
  return { text: text.trim(), rank: false, kind, items: parts.map((p) => p.trim()).filter(Boolean), anonymous: false };
}

export function itemsOk(items: string[]) {
  const keys = new Set(items.map((i) => i.trim().toLowerCase()));
  return (
    items.length >= ITEMS_MIN &&
    items.length <= ITEMS_MAX &&
    keys.size === items.length &&
    items.every((i) => {
      const n = [...i.trim()].length;
      return n >= 1 && n <= ITEM_MAX;
    })
  );
}

export function parseQuestions(content: string): DraftLine[] {
  const seen = new Set<string>();
  return content
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith('#'))
    .map((line) => {
      const { text, rank, kind, items, anonymous } = splitTag(line);
      const len = [...text].length;
      const key = text.toLowerCase();
      const problem: DraftProblem | null =
        len < MIN ? 'short' : len > MAX ? 'long' : seen.has(key) ? 'duplicate' : items && !itemsOk(items) ? 'items' : null;
      seen.add(key);
      return { line, text, rank, kind, items, anonymous, selected: !problem, problem };
    });
}

export const QUESTION_MIN = MIN;
export const QUESTION_MAX = MAX;
