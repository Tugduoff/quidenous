import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ApiError } from '@/lib/api/client';
import { type Queue, questionKeys, reorderQueue } from '@/lib/api/questions';
import { useT } from '@/lib/i18n';

export function useQueueReorder(onStatus: (status: string) => void) {
  const tr = useT();
  const qc = useQueryClient();
  const reorder = useMutation({
    meta: { quiet: true },
    mutationFn: (ids: number[]) => reorderQueue(ids),
    onError: (e) => !(e instanceof ApiError && e.code === 'stale_queue') && onStatus(tr('questions.reorderFailed')),
    onSettled: () => qc.invalidateQueries({ queryKey: questionKeys.queue }),
  });
  return (ids: number[]) => {
    const before = qc.getQueryData<Queue>(questionKeys.queue);
    qc.cancelQueries({ queryKey: questionKeys.queue });
    if (before) {
      const at = new Map(ids.map((id, i) => [id, i]));
      const custom = [...before.custom].sort((a, b) => (at.get(a.id) ?? Infinity) - (at.get(b.id) ?? Infinity));
      qc.setQueryData<Queue>(questionKeys.queue, { ...before, custom });
    }
    reorder.mutate(ids, { onError: () => before && qc.setQueryData(questionKeys.queue, before) });
  };
}
