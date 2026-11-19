import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useEffectEvent } from 'react';
import { takeHandedDraft } from '@/lib/device/files';
import type { DraftLine } from '@/lib/utils/import-draft';

const TAB_SLIDE_MS = 350;

export function useQuestionHandoff(onDraft: (lines: DraftLine[]) => void, focusComposer: () => void) {
  const params = useLocalSearchParams<{ compose?: string; import?: string }>();
  const review = useEffectEvent(onDraft);
  const focus = useEffectEvent(focusComposer);
  useEffect(() => {
    if (!params.compose && !params.import) return;
    const handed = params.import ? takeHandedDraft() : null;
    if (handed) review(handed);
    router.setParams({ compose: undefined, import: undefined });
    if (!params.compose) return;
    const id = setTimeout(() => focus(), TAB_SLIDE_MS);
    return () => clearTimeout(id);
  }, [params.compose, params.import]);
}
