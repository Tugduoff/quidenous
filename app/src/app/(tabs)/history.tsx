import { router } from 'expo-router';
import { Crown } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, AvatarStack, avatarStackWidth, Display } from '@/components/ui';
import type { HistoryItem } from '@/lib/api';
import { useHistory, useMembers } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const DAY = new Intl.DateTimeFormat('fr-FR', { weekday: 'long', day: 'numeric', month: 'long' });
const DAY_YEAR = new Intl.DateTimeFormat('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
/** Solo winner size; tied faces are 4 px smaller so that with their 2 px ring they match it. */
const SOLO = 44;
const TIED = 40;
const MAX = 3;

const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;

/** "Aujourd'hui", "Hier", else "dimanche 8 novembre" (with the year when it is not this one). */
function dayLabel(d: Date) {
  const now = new Date();
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (dayKey(d) === dayKey(now)) return "Aujourd'hui";
  if (dayKey(d) === dayKey(yesterday)) return 'Hier';
  const label = (d.getFullYear() === now.getFullYear() ? DAY : DAY_YEAR).format(d);
  return label[0].toUpperCase() + label.slice(1);
}

/** Consecutive items closed on the same local day (the list is newest first). */
function byDay(items: HistoryItem[]) {
  const days: { key: string; label: string; items: HistoryItem[] }[] = [];
  for (const item of items) {
    const d = new Date(item.closed_at);
    const key = dayKey(d);
    if (days.at(-1)?.key !== key) days.push({ key, label: dayLabel(d), items: [] });
    days.at(-1)!.items.push(item);
  }
  return days;
}

export default function HistoryScreen() {
  const t = useTheme();
  const { data } = useHistory();
  const { byId } = useMembers();
  // Same leading column on every row, as wide as the widest tie on screen.
  const lead = Math.max(SOLO, ...(data ?? []).map((i) => (i.winner_ids.length > 1 ? avatarStackWidth(i.winner_ids.length, TIED, MAX) : 0)));
  return (
    <Screen>
      <Display size={30}>Historique</Display>
      {byDay(data ?? []).map((day) => (
        <View key={day.key} style={{ gap: 10 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{day.label}</Text>
          {day.items.map((item) => {
            const winners = item.winner_ids.map((id) => byId.get(id));
            const winner = winners[0];
            return (
              <Pressable
                key={item.id}
                accessibilityRole="button"
                onPress={() => router.push(`/round/${item.id}`)}
                style={[
                  { flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14, borderRadius: 20, backgroundColor: t.card },
                  t.shadow,
                ]}>
                <View style={{ width: lead, alignItems: 'center' }}>
                  {winners.length > 1 ? (
                    <AvatarStack members={winners} size={TIED} ring={t.card} max={MAX} />
                  ) : (
                    <Avatar member={winner} size={SOLO} />
                  )}
                </View>
                <View style={{ flex: 1, gap: 4 }}>
                  <Text style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 19, color: t.txt }}>{item.question}</Text>
                  <View style={{ flexDirection: 'row', alignItems: 'flex-start', gap: 4 }}>
                    <Crown
                      size={14}
                      color={winners.length === 1 ? (winner?.color ?? t.mut) : t.mut}
                      style={{ flexShrink: 0, marginTop: 1 }}
                    />
                    <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>
                      {winners.map((m) => m?.name).join(', ')}
                    </Text>
                  </View>
                </View>
              </Pressable>
            );
          })}
        </View>
      ))}
    </Screen>
  );
}
