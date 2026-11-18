import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { ApiError } from '@/lib/api/client';
import { importQuestions, questionKeys } from '@/lib/api/questions';
import { roundKeys } from '@/lib/api/rounds';
import { pickQuestionFile } from '@/lib/device/files';
import { useT } from '@/lib/i18n';
import type { DraftLine } from '@/lib/utils/import-draft';

/** Pick a question file, show what's in it, send only what's ticked. */
export function useQuestionImport(onStatus: (status: string) => void) {
  const tr = useT();
  const qc = useQueryClient();
  const [draft, setDraft] = useState<DraftLine[] | null>(null);
  const [mix, setMix] = useState(false);
  const review = (lines: DraftLine[]) => {
    setMix(false);
    setDraft(lines);
  };
  const pick = async () => {
    const lines = await pickQuestionFile();
    if (lines === 'empty') onStatus(tr('questions.emptyFile'));
    else if (lines) review(lines);
  };
  const send = useMutation({
    meta: { quiet: true },
    mutationFn: (lines: string[]) => importQuestions(lines),
    onSuccess: (res) => {
      setDraft(null);
      onStatus(res.skipped ? tr('questions.importedSkipped', { added: res.added, skipped: res.skipped }) : tr('questions.imported', { added: res.added }));
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
    onError: (e) => onStatus(tr(e instanceof ApiError && e.code === 'too_many_questions' ? 'questions.importTooMany' : 'questions.importFailed')),
  });
  return { draft, setDraft, mix, setMix, review, pick, send };
}
