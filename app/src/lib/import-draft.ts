/** Same limits as the server (QUESTION_MIN / QUESTION_MAX). */
const MIN = 8;
const MAX = 200;

export type DraftLine = { text: string; selected: boolean; problem: string | null };

/** A question file as the server will read it, with what's wrong with each line, before anything is sent. */
export function parseQuestions(content: string): DraftLine[] {
  const seen = new Set<string>();
  return content
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith('#'))
    .map((text) => {
      const len = [...text].length;
      const key = text.toLowerCase();
      const problem = len < MIN ? 'trop courte' : len > MAX ? 'trop longue' : seen.has(key) ? 'en double' : null;
      seen.add(key);
      return { text, selected: !problem, problem };
    });
}
