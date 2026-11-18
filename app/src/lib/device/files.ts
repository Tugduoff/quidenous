import * as DocumentPicker from 'expo-document-picker';
import { type DraftLine, parseQuestions } from '@/lib/utils/import-draft';
import { readUriText } from '@/lib/api/client';

/** Pick a text file and read it whole (a pack file's header too): null when cancelled. */
export async function pickTextFile(): Promise<string | null> {
  const picked = await DocumentPicker.getDocumentAsync({ type: ['text/plain', 'text/*'], copyToCacheDirectory: true });
  if (picked.canceled || !picked.assets[0]) return null;
  const asset = picked.assets[0];
  return asset.file ? await asset.file.text() : await readUriText(asset.uri);
}

/** Pick a question file and read it: its lines, 'empty' when there's nothing in it, null when cancelled. */
export async function pickQuestionFile(): Promise<DraftLine[] | 'empty' | null> {
  const content = await pickTextFile();
  if (content == null) return null;
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
