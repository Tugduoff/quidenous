import { Text, View } from 'react-native';
import { type GlobalCount } from '@/lib/api/global';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { type Member } from '@/lib/api/group';

export function YesterdayWinner({ tally, byId }: { tally: GlobalCount[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const best = tally[0]?.count ?? 0;
  const first = tally.filter((c) => c.count === best && best > 0).map((c) => byId.get(c.target_id));
  if (!first.length) return <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>{tr('globalQuestion.noVotesYesterday')}</Text>;
  return (
    <View style={{ gap: 8 }}>
      {first.map((m, i) => (
        <View key={m?.id ?? `x${i}`} style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <Avatar member={m} size={32} />
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
            {m?.name ?? '?'}
          </Text>
          <Text style={{ fontFamily: t.font.display, fontSize: 18, color: t.txt }}>{best}</Text>
        </View>
      ))}
    </View>
  );
}
