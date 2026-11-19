import { Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { StreakBadge } from '@/components/streaks/streaks';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { type Member } from '@/lib/api/group';
import { type Round } from '@/lib/api/rounds';
import { NudgeButton } from '@/components/vote/nudge-button';
import { SkipButton } from '@/components/vote/skip-button';

export function PendingRow({ member: m, round, canSkip, big }: { member: Member; round: Round; canSkip: boolean; big?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const skipped = round.skipped_ids.includes(m.id);
  const away = !skipped && !!round.away_ids?.includes(m.id);
  return (
    <Animated.View exiting={motion.exit} layout={motion.layout} style={{ flexDirection: 'row', alignItems: 'center', gap: big ? 12 : 10 }}>
      <Avatar member={m} size={big ? 44 : 32} />
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: big ? t.font.bold : t.font.semibold, fontSize: big ? 17 : 15, color: t.txt }}>
        {m.name}
      </Text>
      <StreakBadge id={m.id} />
      {(skipped || away) && (
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr(skipped ? 'vote.waiting.skipped' : 'vote.waiting.away')}</Text>
      )}
      {canSkip && m.id !== me?.id && !skipped && !away && <SkipButton member={m} round={round} />}
      {m.id !== me?.id && <NudgeButton member={m} roundId={round.id} />}
    </Animated.View>
  );
}
