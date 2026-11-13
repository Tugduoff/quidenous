import * as DocumentPicker from 'expo-document-picker';

/** Same limits as the server (QUESTION_MIN / QUESTION_MAX). */
const MIN = 8;
const MAX = 200;

/** Choose questions: 2 to 4 items, each 1 to 40 characters (same as the server). */
export const ITEMS_MIN = 2;
export const ITEMS_MAX = 4;
export const ITEM_MAX = 40;

/** What's wrong with a line (shown as `questions.problem.*`). `items`: a choose line whose items don't hold. */
export type DraftProblem = 'short' | 'long' | 'duplicate' | 'items';

export type DraftKind = 'pick' | 'rank' | 'choose';

/** `line` is what gets sent (tag included); `text` is the question as people will see it; `items` a choose line's items. */
export type DraftLine = {
  line: string;
  text: string;
  rank: boolean;
  kind: DraftKind;
  items: string[] | null;
  selected: boolean;
  problem: DraftProblem | null;
};

/** Tags the built-in deck's vibes use (server/deck/vibes.json): dropped from the question like the server does. */
const VIBE_TAGS = ['potes', 'famille', 'boulot', 'pimente'];

/**
 * Same rule as the server (`deck::parse_tagged`): leading tags in any order, `[classement]` / `[rank]` make a ranking
 * question, `[choix]` / `[choose]` a choose question (`[choix] Qui prendrait quoi ? | A | B`: the question stops at the
 * first `|`, each other part is an item), vibe tags are dropped; the first unknown `[…]` is part of the question.
 */
export function splitTag(line: string): { text: string; rank: boolean; kind: DraftKind; items: string[] | null } {
  let rest = line.trim();
  let kind: DraftKind = 'pick';
  for (;;) {
    const m = /^\[([^\]]*)\]/.exec(rest);
    if (!m) break;
    const tag = m[1].trim().toLowerCase();
    if (tag === 'classement' || tag === 'rank') kind = 'rank';
    else if (tag === 'choix' || tag === 'choose') kind = 'choose';
    else if (!VIBE_TAGS.includes(tag)) break;
    rest = rest.slice(m[0].length).trimStart();
  }
  if (kind !== 'choose') return { text: rest, rank: kind === 'rank', kind, items: null };
  const [text, ...parts] = rest.split('|');
  return { text: text.trim(), rank: false, kind, items: parts.map((p) => p.trim()).filter(Boolean) };
}

/** 2 to 4 items, each 1 to 40 characters, no two alike (case-insensitive). */
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

/** A question file as the server will read it, with what's wrong with each line, before anything is sent. */
export function parseQuestions(content: string): DraftLine[] {
  const seen = new Set<string>();
  return content
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith('#'))
    .map((line) => {
      const { text, rank, kind, items } = splitTag(line);
      const len = [...text].length;
      const key = text.toLowerCase();
      const problem: DraftProblem | null =
        len < MIN ? 'short' : len > MAX ? 'long' : seen.has(key) ? 'duplicate' : items && !itemsOk(items) ? 'items' : null;
      seen.add(key);
      return { line, text, rank, kind, items, selected: !problem, problem };
    });
}

/** Pick a question file and read it: its lines, 'empty' when there's nothing in it, null when cancelled. */
export async function pickQuestionFile(): Promise<DraftLine[] | 'empty' | null> {
  const picked = await DocumentPicker.getDocumentAsync({ type: ['text/plain', 'text/*'], copyToCacheDirectory: true });
  if (picked.canceled || !picked.assets[0]) return null;
  const asset = picked.assets[0];
  const content = asset.file ? await asset.file.text() : await (await fetch(asset.uri)).text();
  const lines = parseQuestions(content);
  return lines.length ? lines : 'empty';
}

/**
 * A file picked elsewhere (the Vote tab, right in the tap, as browsers want), handed to the Questions tab to review.
 * Taken once.
 */
let handed: DraftLine[] | null = null;
export const handOffDraft = (lines: DraftLine[]) => {
  handed = lines;
};
export const takeHandedDraft = () => {
  const lines = handed;
  handed = null;
  return lines;
};
