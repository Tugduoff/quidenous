import { router } from 'expo-router';
import { Crown, Download, Hourglass, PartyPopper, Pin } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Screen } from '@/components/screen';
import { Avatar, AvatarStack, avatarStackWidth, Display, PressableScale, QuestionText } from '@/components/ui';
import type { HistoryItem, Member } from '@/lib/api';
import { isMilestone } from '@/lib/milestones';
import { motion } from '@/lib/motion';
import { downloadHistory } from '@/lib/share';
import { useHistory, useMembers, usePinned } from '@/lib/queries';
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

function HistoryCard({ item, lead, byId, order }: { item: HistoryItem; lead: number; byId: Map<number, Member>; order?: number }) {
  const t = useTheme();
  const winners = item.winner_ids.map((id) => byId.get(id));
  const winner = winners[0];
  return (
    <Animated.View entering={order != null ? motion.stagger(order) : undefined}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        onPress={() => router.push(`/round/${item.id}`)}
        style={[{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14, borderRadius: 20, backgroundColor: t.card }, t.shadow]}>
        <View style={{ width: lead, alignItems: 'center' }}>
          {winners.length > 1 ? <AvatarStack members={winners} size={TIED} ring={t.card} max={MAX} /> : <Avatar member={winner} size={SOLO} />}
        </View>
        <View style={{ flex: 1, gap: 4 }}>
          <QuestionText
            text={item.question}
            rank={item.kind === 'rank'}
            style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 19, color: t.txt }}
          />
          <View style={{ flexDirection: 'row', alignItems: 'flex-start', gap: 4 }}>
            <Crown size={14} color={winners.length === 1 ? (winner?.color ?? t.mut) : t.mut} style={{ flexShrink: 0, marginTop: 1 }} />
            <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>
              {winners.map((m) => m?.name).join(', ')}
            </Text>
          </View>
        </View>
        {(item.pinned || isMilestone(item.number)) && (
          <View style={{ alignSelf: 'flex-start', alignItems: 'flex-end', gap: 6, flexShrink: 0 }}>
            {item.pinned && <Pin size={16} color={t.accInk} />}
            {/* A milestone question: tap to replay its party. */}
            {isMilestone(item.number) && (
              <View
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 4,
                  paddingHorizontal: 8,
                  height: 22,
                  borderRadius: t.radius.pill,
                  backgroundColor: t.accSoft,
                }}>
                <PartyPopper size={12} color={t.accInk} />
                <Text style={{ fontFamily: t.font.bold, fontSize: 11, color: t.accInk }}>#{item.number}</Text>
              </View>
            )}
          </View>
        )}
      </PressableScale>
    </Animated.View>
  );
}

function SectionLabel({ children }: { children: string }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{children}</Text>;
}

/** No result yet: one line, and the way back to the vote. */
function Empty() {
  const t = useTheme();
  return (
    <Animated.View entering={motion.enter} style={{ flex: 1, alignItems: 'center', justifyContent: 'center', gap: 12, paddingBottom: 60 }}>
      <View style={{ width: 56, height: 56, borderRadius: 28, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <Hourglass size={24} color={t.mut} />
      </View>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut, textAlign: 'center' }}>Les résultats arriveront ici.</Text>
      <Pressable accessibilityRole="button" onPress={() => router.navigate('/')} hitSlop={8}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.accInk }}>Aller voter</Text>
      </Pressable>
    </Animated.View>
  );
}

export default function HistoryScreen() {
  const t = useTheme();
  const history = useHistory();
  const { data: pinned } = usePinned();
  const data = history.data?.pages.flat();
  const { byId } = useMembers();
  // The first screenful cascades in once; refetches and later pages just appear.
  const [intro, setIntro] = useState(true);
  const loaded = !!data;
  useEffect(() => {
    if (!loaded) return;
    const done = setTimeout(() => setIntro(false), 900);
    return () => clearTimeout(done);
  }, [loaded]);
  let row = 0;
  const order = () => (intro ? row++ : undefined);
  const more = () => {
    if (history.hasNextPage && !history.isFetchingNextPage) history.fetchNextPage();
  };
  // Same leading column on every row, as wide as the widest tie on screen.
  const lead = Math.max(
    SOLO,
    ...[...(pinned ?? []), ...(data ?? [])].map((i) => (i.winner_ids.length > 1 ? avatarStackWidth(i.winner_ids.length, TIED, MAX) : 0)),
  );
  return (
    <Screen onEndReached={more}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Display size={30}>Historique</Display>
        {/* Nothing to export yet. */}
        {!!(data?.length || pinned?.length) && (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel="Exporter l'historique"
            onPress={downloadHistory}
            hitSlop={8}
            style={[
              {
                flexDirection: 'row',
                alignItems: 'center',
                gap: 6,
                height: 36,
                paddingHorizontal: 14,
                borderRadius: t.radius.pill,
                backgroundColor: t.card,
              },
              t.shadow,
            ]}>
            <Download size={16} color={t.txt} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>Exporter</Text>
          </PressableScale>
        )}
      </View>
      {!!pinned?.length && (
        <View style={{ gap: 10 }}>
          <SectionLabel>Épinglés</SectionLabel>
          {pinned.map((item) => (
            <HistoryCard key={item.id} item={item} lead={lead} byId={byId} order={order()} />
          ))}
        </View>
      )}
      {byDay(data ?? []).map((day) => (
        <View key={day.key} style={{ gap: 10 }}>
          <SectionLabel>{day.label}</SectionLabel>
          {day.items.map((item) => (
            <HistoryCard key={item.id} item={item} lead={lead} byId={byId} order={order()} />
          ))}
        </View>
      ))}
      {data && data.length === 0 && !pinned?.length && <Empty />}
      {history.isFetchingNextPage && <ActivityIndicator color={t.mut} style={{ paddingVertical: 8 }} />}
    </Screen>
  );
}
