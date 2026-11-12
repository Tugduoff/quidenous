/** Same limits as the server (QUESTION_MIN / QUESTION_MAX). */
const MIN = 8;
const MAX = 200;

/** `line` is what gets sent (tag included); `text` is the question as people will see it. */
export type DraftLine = { line: string; text: string; rank: boolean; selected: boolean; problem: string | null };

const TAGS = ['[classement]', '[rank]'];

/** Same rule as the server: a `[classement]` line is a ranking question, the tag isn't part of it. */
function split(line: string) {
  const tag = TAGS.find((t) => line.toLowerCase().startsWith(t));
  return tag ? { text: line.slice(tag.length).trim(), rank: true } : { text: line, rank: false };
}

/** A question file as the server will read it, with what's wrong with each line, before anything is sent. */
export function parseQuestions(content: string): DraftLine[] {
  const seen = new Set<string>();
  return content
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith('#'))
    .map((line) => {
      const { text, rank } = split(line);
      const len = [...text].length;
      const key = text.toLowerCase();
      const problem = len < MIN ? 'trop courte' : len > MAX ? 'trop longue' : seen.has(key) ? 'en double' : null;
      seen.add(key);
      return { line, text, rank, selected: !problem, problem };
    });
}
