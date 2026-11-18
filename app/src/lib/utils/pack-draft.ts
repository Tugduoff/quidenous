import type { PackQuestion } from '@/lib/api/packs';
import { itemsOk, QUESTION_MAX, QUESTION_MIN } from '@/lib/utils/import-draft';

/** The server refuses a pack under 5 questions: said before the tap. */
export const PACK_MIN = 5;

export const questionKey = (text: string) => text.trim().toLowerCase();

/** `more` after `list`, without a question already there (case-insensitive), out of bounds, or a choose one with bad items. */
export function mergeQuestions(list: PackQuestion[], more: PackQuestion[]): PackQuestion[] {
  const seen = new Set(list.map((q) => q.text.trim().toLowerCase()));
  const out = [...list];
  for (const q of more) {
    const text = q.text.trim();
    const key = text.toLowerCase();
    const len = [...text].length;
    if (seen.has(key) || len < QUESTION_MIN || len > QUESTION_MAX) continue;
    if (q.kind === 'choose') {
      const items = (q.items ?? []).map((x) => x.trim());
      if (!itemsOk(items)) continue;
      out.push({ text, kind: 'choose', items });
    } else out.push({ text, kind: q.kind === 'rank' ? 'rank' : 'pick', anonymous: !!q.anonymous });
    seen.add(key);
  }
  return out;
}

/** The first `max` characters (whole characters, not UTF-16 halves). */
export const clip = (s: string, max: number) => [...s.trim()].slice(0, max).join('');
