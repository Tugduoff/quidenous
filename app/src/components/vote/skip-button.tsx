import { useMutation, useQueryClient } from '@tanstack/react-query';
import { SkipForward } from 'lucide-react-native';
import { useState } from 'react';
import { Text } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { ConfirmModal } from '@/components/ui/dialog';
import { type Member } from '@/lib/api/group';
import { type Round, roundKeys, setSkip } from '@/lib/api/rounds';

export function SkipButton({ member, round }: { member: Member; round: Round }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const state = round.skips.find((s) => s.target_id === member.id);
  const mine = !!state?.mine;
  const [asking, setAsking] = useState(false);
  const skip = useMutation({
    mutationFn: () => setSkip(member.id, !mine),
    onSettled: () => {
      setAsking(false);
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
  });
  return (
    <>
      <ConfirmModal
        visible={asking}
        title={tr('vote.skip.title', { name: member.name })}
        message={tr('vote.skip.message')}
        confirmLabel={tr('vote.skip.confirm')}
        busy={skip.isPending}
        onConfirm={() => skip.mutate()}
        onCancel={() => setAsking(false)}
      />
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr(mine ? 'vote.skip.undo' : 'vote.skip.label', { name: member.name })}
        disabled={skip.isPending}
        onPress={() => (mine ? skip.mutate() : setAsking(true))}
        hitSlop={6}
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 4,
          minWidth: 36,
          height: 36,
          paddingHorizontal: state ? 10 : 0,
          borderRadius: 18,
          backgroundColor: mine ? t.accSoft : t.card2,
        }}>
        <SkipForward size={16} color={mine ? t.accInk : t.mut} />
        {state && (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: mine ? t.accInk : t.mut }}>
            {state.count}/{round.skip_needed}
          </Text>
        )}
      </PressableScale>
    </>
  );
}
