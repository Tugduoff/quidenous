import { ConfirmModal } from '@/components/ui/dialog';
import { answerConfirm, useAsking } from '@/lib/confirm';

export function ConfirmHost() {
  const ask = useAsking();
  return (
    <ConfirmModal
      visible={!!ask}
      title={ask?.title ?? ''}
      message={ask?.message}
      confirmLabel={ask?.action ?? ''}
      tone={ask?.tone}
      onConfirm={() => answerConfirm(true)}
      onCancel={() => answerConfirm(false)}
    />
  );
}
