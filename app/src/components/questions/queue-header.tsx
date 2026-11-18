import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ListX, Shuffle } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { PressableScale } from '@/components/ui/pressable-scale';
import { type Selection, SelectButton, SelectionHeader } from '@/components/ui/selection';
import { useFeatures } from '@/lib/api/features';
import { useGroup } from '@/lib/api/group';
import { emptyQueue, questionKeys, shuffleQueue } from '@/lib/api/questions';
import { roundKeys } from '@/lib/api/rounds';
import { askConfirm } from '@/lib/confirm';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function QueueHeader({
  count,
  locked,
  status,
  onStatus,
  sel,
  canSelect,
}: {
  count: number;
  locked: boolean;
  status: string | null;
  onStatus: (status: string) => void;
  sel: Selection<number>;
  canSelect: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const { clearQueue } = useFeatures();
  const shuffle = useMutation({
    meta: { quiet: true },
    mutationFn: () => shuffleQueue(),
    onSuccess: () => {
      onStatus(tr('questions.shuffled'));
      qc.invalidateQueries({ queryKey: questionKeys.queue });
    },
    onError: () => onStatus(tr('questions.shuffleFailed')),
  });
  const clear = useMutation({
    meta: { quiet: true },
    mutationFn: () => emptyQueue(),
    onSuccess: (res) => onStatus(tr('questions.cleared', { count: res?.deleted ?? 0 })),
    onError: () => onStatus(tr('questions.clearFailed')),
    onSettled: () => {
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
  });
  const askClear = async () => {
    if (await askConfirm(tr('questions.clearTitle', { count }), undefined, tr('common.delete'))) clear.mutate();
  };
  if (sel.on)
    return (
      <View style={{ marginTop: 6 }}>
        <SelectionHeader sel={sel} />
      </View>
    );
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 6 }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('questions.upcoming')}</Text>
      <View style={{ paddingHorizontal: 8, height: 22, borderRadius: 11, justifyContent: 'center', backgroundColor: t.card2 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{count}</Text>
      </View>
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
        {status ?? ''}
      </Text>
      {!locked && count > 1 && (
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('questions.shuffleUpcoming')}
          disabled={shuffle.isPending}
          onPress={() => shuffle.mutate()}
          hitSlop={8}
          style={{
            width: 36,
            height: 36,
            borderRadius: 18,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: t.accSoft,
            opacity: shuffle.isPending ? 0.5 : 1,
          }}>
          <Shuffle size={18} color={t.accInk} />
        </PressableScale>
      )}
      {clearQueue && group?.my_role === 'owner' && count > 0 && (
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('questions.clearQueue')}
          disabled={clear.isPending}
          onPress={askClear}
          hitSlop={8}
          style={{
            width: 36,
            height: 36,
            borderRadius: 18,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: t.card2,
            opacity: clear.isPending ? 0.5 : 1,
          }}>
          <ListX size={18} color={t.mut} />
        </PressableScale>
      )}
      {canSelect && <SelectButton size={36} onPress={() => sel.start()} />}
    </View>
  );
}
