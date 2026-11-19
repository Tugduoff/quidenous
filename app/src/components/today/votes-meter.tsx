import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function VotesMeter({ votes, needed }: { votes: number; needed: number }) {
  const t = useTheme();
  const tr = useT();
  const k = needed > 0 ? Math.min(1, votes / needed) : 0;
  return (
    <View
      accessible
      accessibilityLabel={tr('today.votesA11y', { votes, needed })}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
      <View style={{ width: 56, height: 6, borderRadius: 3, backgroundColor: t.card2, overflow: 'hidden' }}>
        <View style={{ width: `${k * 100}%`, height: 6, borderRadius: 3, backgroundColor: t.accInk }} />
      </View>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, fontVariant: ['tabular-nums'] }}>
        {votes}/{needed}
      </Text>
    </View>
  );
}
