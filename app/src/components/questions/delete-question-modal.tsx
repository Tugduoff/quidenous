import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ConfirmModal } from '@/components/ui/dialog';
import { deleteQuestion, questionKeys } from '@/lib/api/questions';
import { useT } from '@/lib/i18n';
import { tidy } from '@/lib/utils/text';

/** Its author or the room owner takes a question out before it's asked. */
export function DeleteQuestionModal({ question, onClose }: { question: { id: number; text: string } | null; onClose: () => void }) {
  const tr = useT();
  const qc = useQueryClient();
  const remove = useMutation({
    mutationFn: (id: number) => deleteQuestion(id),
    onSettled: () => {
      onClose();
      qc.invalidateQueries({ queryKey: questionKeys.queue });
    },
  });
  return (
    <ConfirmModal
      visible={!!question}
      title={tr('questions.deleteTitle')}
      message={question ? tr('questions.deleteBody', { text: tidy(question.text) }) : undefined}
      confirmLabel={tr('common.delete')}
      busy={remove.isPending}
      onCancel={onClose}
      onConfirm={() => question && remove.mutate(question.id)}
    />
  );
}
