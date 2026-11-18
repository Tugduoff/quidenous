import { useMutation } from '@tanstack/react-query';
import { BellRing, Check } from 'lucide-react-native';
import { useState } from 'react';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { type Member } from '@/lib/api/group';
import { nudgeMember } from '@/lib/api/rounds';

/** Sends a "hurry up" push to someone who has not voted yet. Once per question. */
export function NudgeButton({ member, roundId }: { member: Member; roundId: number }) {
  const t = useTheme();
  const tr = useT();
  const [sentFor, setSentFor] = useState<number | null>(null);
  const sent = sentFor === roundId;
  const setSent = () => setSentFor(roundId);
  const nudge = useMutation({
    mutationFn: () => nudgeMember(member.id),
    // Nudged already (another phone of mine, a lost answer): same as sent. Any other failure: a toast, the bell stays.
    meta: { quiet: (e) => e instanceof ApiError && e.code === 'already_nudged' },
    onSuccess: setSent,
    onError: (e) => e instanceof ApiError && e.code === 'already_nudged' && setSent(),
  });
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('vote.nudge', { name: member.name })}
      disabled={sent || nudge.isPending}
      onPress={() => nudge.mutate()}
      style={{
        width: 36,
        height: 36,
        borderRadius: 18,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: sent ? t.card2 : t.accSoft,
      }}>
      {sent ? <Check size={18} color={t.mut} /> : <BellRing size={18} color={t.accInk} />}
    </PressableScale>
  );
}
