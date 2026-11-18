import * as DocumentPicker from 'expo-document-picker';
import { type DraftLine, parseQuestions } from '@/lib/utils/import-draft';
import { readUriText } from '@/lib/api/client';

export async function pickTextFile(): Promise<string | null> {
  const picked = await DocumentPicker.getDocumentAsync({ type: ['text/plain', 'text/*'], copyToCacheDirectory: true });
  if (picked.canceled || !picked.assets[0]) return null;
  const asset = picked.assets[0];
  return asset.file ? await asset.file.text() : await readUriText(asset.uri);
}

export async function pickQuestionFile(): Promise<DraftLine[] | 'empty' | null> {
  const content = await pickTextFile();
  if (content == null) return null;
  const lines = parseQuestions(content);
  return lines.length ? lines : 'empty';
}

let handed: DraftLine[] | null = null;

export const handOffDraft = (lines: DraftLine[]) => {
  handed = lines;
};

export const takeHandedDraft = () => {
  const lines = handed;
  handed = null;
  return lines;
};
