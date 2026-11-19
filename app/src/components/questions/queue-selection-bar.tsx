import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowDownToLine, ArrowUpToLine, Trash2 } from 'lucide-react-native';
import { confirmBatch, type Selection, SelectionBar } from '@/components/ui/selection';
import { deleteQuestions, questionKeys, type QueuedQuestion } from '@/lib/api/questions';
import { roundKeys } from '@/lib/api/rounds';
import { useT } from '@/lib/i18n';

export function QueueSelectionBar({
  sel,
  queue,
  canMove,
  canDeleteMany,
  canDelete,
  onReorder,
  onStatus,
}: {
  sel: Selection<number>;
  queue: QueuedQuestion[] | undefined;
  canMove: boolean;
  canDeleteMany: boolean;
  canDelete: (q: QueuedQuestion) => boolean;
  onReorder: (ids: number[]) => void;
  onStatus: (status: string) => void;
}) {
  const tr = useT();
  const qc = useQueryClient();
  const moveSelected = (first: boolean) => {
    if (!queue || !sel.count) return;
    const picked = new Set(sel.selected);
    const rest = queue.map((q) => q.id).filter((id) => !picked.has(id));
    const count = sel.count;
    onReorder(first ? [...sel.selected, ...rest] : [...rest, ...sel.selected]);
    sel.cancel();
    onStatus(tr(first ? 'questions.movedFirst' : 'questions.movedLast', { count }));
  };
  const removeMany = useMutation({
    meta: { quiet: true },
    mutationFn: (ids: number[]) => deleteQuestions(ids),
    onSuccess: (res, ids) =>
      onStatus(
        res.skipped.length
          ? tr('common.select.partial', { done: res.deleted.length, count: ids.length })
          : tr('questions.cleared', { count: res.deleted.length }),
      ),
    onError: () => onStatus(tr('common.failed')),
    onSettled: () => {
      sel.cancel();
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
  });
  const mineSelected = sel.selected.filter((id) => queue?.some((q) => q.id === id && canDelete(q)));
  const askDeleteSelected = async () => {
    const count = mineSelected.length;
    const others = sel.count - count;
    if (!count) return;
    const message = others ? tr('questions.deleteSelectedOthers', { count: others }) : undefined;
    if (await confirmBatch(tr('questions.deleteSelectedTitle', { count }), tr('common.delete'), message)) removeMany.mutate(mineSelected);
  };
  return (
    <SelectionBar
      sel={sel}
      actions={[
        ...(canMove
          ? [
              { icon: ArrowUpToLine, label: tr('questions.moveFirst'), onPress: () => moveSelected(true) },
              { icon: ArrowDownToLine, label: tr('questions.moveLast'), onPress: () => moveSelected(false) },
            ]
          : []),
        ...(canDeleteMany
          ? [
              {
                icon: Trash2,
                label: tr('common.delete'),
                tone: 'danger' as const,
                onPress: askDeleteSelected,
                disabled: !mineSelected.length,
                busy: removeMany.isPending,
              },
            ]
          : []),
      ]}
    />
  );
}
