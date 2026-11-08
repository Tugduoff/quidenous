import { router } from 'expo-router';
import { Crown } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Display } from '@/components/ui';
import { useHistory, useMembers } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const DAY = new Intl.DateTimeFormat('fr-FR', { weekday: 'short', day: 'numeric', month: 'short' });

export default function HistoryScreen() {
  const t = useTheme();
  const { data } = useHistory();
  const { byId, members } = useMembers();
  return (
    <Screen>
      <Display size={30}>Historique</Display>
      {data?.map((item) => {
        const winner = byId.get(item.winner_ids[0]);
        return (
          <Pressable
            key={item.id}
            accessibilityRole="button"
            onPress={() => router.push(`/round/${item.id}`)}
            style={[{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14, borderRadius: 20, backgroundColor: t.card }, t.shadow]}>
            <Avatar member={winner} size={44} />
            <View style={{ flex: 1, gap: 4 }}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 19, color: t.txt }}>{item.question}</Text>
              <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
                <Crown size={14} color={winner?.color ?? t.mut} />
                <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
                  {item.winner_ids.map((id) => byId.get(id)?.name).join(', ')} · {item.top_count}/{members.length} ·{' '}
                  {DAY.format(new Date(item.closed_at))}
                </Text>
              </View>
            </View>
          </Pressable>
        );
      })}
    </Screen>
  );
}
