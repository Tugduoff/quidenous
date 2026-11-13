import * as DocumentPicker from 'expo-document-picker';

/** Same limits as the server (QUESTION_MIN / QUESTION_MAX). */
const MIN = 8;
const MAX = 200;

/** What's wrong with a line (shown as `questions.problem.*`). */
export type DraftProblem = 'short' | 'long' | 'duplicate';

/** `line` is what gets sent (tag included); `text` is the question as people will see it. */
export type DraftLine = { line: string; text: string; rank: boolean; selected: boolean; problem: DraftProblem | null };

const TAGS = ['[classement]', '[rank]'];

/** Same rule as the server: a `[classement]` (or `[rank]`) line is a ranking question, the tag isn't part of it. */
export function splitTag(line: string) {
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
      const { text, rank } = splitTag(line);
      const len = [...text].length;
      const key = text.toLowerCase();
      const problem: DraftProblem | null = len < MIN ? 'short' : len > MAX ? 'long' : seen.has(key) ? 'duplicate' : null;
      seen.add(key);
      return { line, text, rank, selected: !problem, problem };
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
