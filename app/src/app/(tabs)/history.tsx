import { router } from 'expo-router';
import { Cable, Crown, Download, MessageSquareQuote, PartyPopper, Pin, Target } from 'lucide-react-native';
import { useEffect, useMemo, useState } from 'react';
import { ActivityIndicator, Pressable, SectionList, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { SafeAreaView } from 'react-native-safe-area-context';
import { AnonymousMark } from '@/components/vote/anonymous';
import { Mascot } from '@/components/mascot/mascot';
import { QueryState } from '@/components/ui/query-state';
import { dateFormat, t as translate, useT } from '@/lib/i18n';
import { isMilestone } from '@/lib/utils/milestones';
import { motion } from '@/lib/motion';
import { downloadHistory } from '@/lib/device/share';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar, AvatarStack, avatarStackWidth } from '@/components/ui/avatar';
import { Display, QuestionText } from '@/components/ui/text';
import { type Member, useMembers } from '@/lib/api/group';
import { type HistoryItem, useHistory, usePinned } from '@/lib/api/history';

/** Solo winner size; tied faces are 4 px smaller so that with their 2 px ring they match it. */
const SOLO = 44;
const TIED = 40;
/** A tie shows two faces at most (the second one "+n" past two): the names are spelled out under the question. */
const MAX = 2;

const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;

/** "Aujourd'hui", "Hier", else "dimanche 8 novembre" (with the year when it is not this one). */
function dayLabel(d: Date) {
  const now = new Date();
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (dayKey(d) === dayKey(now)) return translate('history.today');
  if (dayKey(d) === dayKey(yesterday)) return translate('history.yesterday');
  const sameYear = d.getFullYear() === now.getFullYear();
  const label = dateFormat({ weekday: 'long', day: 'numeric', month: 'long', ...(sameYear ? {} : { year: 'numeric' }) }).format(d);
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

function HistoryCard({ item, byId, order }: { item: HistoryItem; byId: Map<number, Member>; order?: number }) {
  const t = useTheme();
  const tr = useT();
  const winners = item.winner_ids.map((id) => byId.get(id));
  const winner = winners[0];
  // A choose question's "winners" are its best guessers (none when nobody guessed right); a said one's, its top scorers.
  const choose = item.kind === 'choose';
  const said = item.kind === 'said';
  const Mark = choose ? Target : Crown;
  const Empty = said ? MessageSquareQuote : Cable;
  // Each row as wide as its own faces: a tie elsewhere doesn't squeeze this title.
  const lead = winners.length > 1 ? avatarStackWidth(winners.length, TIED, MAX) : SOLO;
  return (
    <Animated.View entering={order != null ? motion.stagger(order) : undefined}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        onPress={() => router.push(`/round/${item.id}`)}
        style={[{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14, borderRadius: 20, backgroundColor: t.card }, t.shadow]}>
        <View style={{ width: lead, alignItems: 'center' }}>
          {winners.length > 1 ? (
            <AvatarStack members={winners} size={TIED} ring={t.card} max={MAX} />
          ) : winner || !(choose || said) ? (
            <Avatar member={winner} size={SOLO} />
          ) : (
            <View
              style={{ width: SOLO, height: SOLO, borderRadius: SOLO / 2, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
              <Empty size={20} color={t.mut} />
            </View>
          )}
        </View>
        <View style={{ flex: 1, gap: 4 }}>
          <QuestionText
            text={item.question}
            rank={item.kind === 'rank'}
            choose={choose}
            said={said}
            style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 19, color: t.txt }}
          />
          <View style={{ flexDirection: 'row', alignItems: 'flex-start', gap: 4 }}>
            <Mark size={14} color={winners.length === 1 ? (winner?.color ?? t.mut) : t.mut} style={{ flexShrink: 0, marginTop: 1 }} />
            <Text
              numberOfLines={1}
              style={{ flex: item.anonymous ? undefined : 1, flexShrink: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>
              {winners.length ? winners.map((m) => m?.name).join(', ') : choose ? tr('history.nobodyGuessed') : said ? tr('said.result.nobody') : ''}
            </Text>
            {item.anonymous && (
              <View style={{ marginTop: 1.5, marginLeft: 2 }}>
                <AnonymousMark size={13} />
              </View>
            )}
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
  const tr = useT();
  return (
    <Animated.View entering={motion.enter} style={{ flex: 1, alignItems: 'center', justifyContent: 'center', gap: 12, paddingBottom: 60 }}>
      {/* Roupi waits for the first results the only way he knows. */}
      <Mascot who="roupi" mood="sleepy" size={128} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut, textAlign: 'center' }}>{tr('history.empty')}</Text>
      <Pressable accessibilityRole="button" onPress={() => router.navigate('/')} hitSlop={8}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.accInk }}>{tr('history.goVote')}</Text>
      </Pressable>
    </Animated.View>
  );
}

type Section = { key: string; title: string; data: HistoryItem[] };

export default function HistoryScreen() {
  const t = useTheme();
  const tr = useT();
  const history = useHistory();
  const { data: pinned } = usePinned();
  const data = useMemo(() => history.data?.pages.flat(), [history.data]);
  const { byId } = useMembers();
  // The first screenful cascades in once; refetches and later pages just appear.
  const [intro, setIntro] = useState(true);
  const loaded = !!data;
  useEffect(() => {
    if (!loaded) return;
    const done = setTimeout(() => setIntro(false), 900);
    return () => clearTimeout(done);
  }, [loaded]);
  const more = () => {
    if (history.hasNextPage && !history.isFetchingNextPage) history.fetchNextPage();
  };
  // A pin or unpin updates the pinned list alone: it decides the pin on every row.
  const pinnedIds = useMemo(() => (pinned ? new Set(pinned.map((i) => i.id)) : null), [pinned]);
  const sections = useMemo<Section[]>(() => {
    const days = byDay(data ?? []).map((d) => ({ key: d.key, title: d.label, data: d.items.map((i) => (pinnedIds ? { ...i, pinned: pinnedIds.has(i.id) } : i)) }));
    return pinned?.length ? [{ key: 'pinned', title: tr('history.pinned'), data: pinned }, ...days] : days;
  }, [data, pinned, pinnedIds, tr]);
  return (
    <SafeAreaView edges={['top']} style={{ flex: 1, backgroundColor: t.bg }}>
      <SectionList
        sections={sections}
        keyExtractor={(item) => String(item.id)}
        stickySectionHeadersEnabled={false}
        contentContainerStyle={{ padding: 20, flexGrow: 1 }}
        onEndReached={more}
        onEndReachedThreshold={1}
        initialNumToRender={12}
        windowSize={7}
        ItemSeparatorComponent={() => <View style={{ height: 10 }} />}
        SectionSeparatorComponent={() => <View style={{ height: 6 }} />}
        ListHeaderComponent={
          <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
            <Display size={30}>{tr('history.title')}</Display>
            {/* Nothing to export yet. */}
            {!!(data?.length || pinned?.length) && (
              <PressableScale
                accessibilityRole="button"
                accessibilityLabel={tr('history.export')}
                onPress={downloadHistory}
                hitSlop={8}
                style={[{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
                <Download size={18} color={t.txt} />
              </PressableScale>
            )}
          </View>
        }
        renderSectionHeader={({ section }) => (
          <View style={{ paddingTop: 10 }}>
            <SectionLabel>{section.title}</SectionLabel>
          </View>
        )}
        renderItem={({ item, index }) => (
          <HistoryCard item={item} byId={byId} order={intro && index < 6 ? index : undefined} />
        )}
        ListFooterComponent={
          <View>
            <QueryState query={history} />
            {data && data.length === 0 && !pinned?.length && <Empty />}
            {history.isFetchingNextPage && <ActivityIndicator color={t.mut} style={{ paddingVertical: 8 }} />}
          </View>
        }
      />
    </SafeAreaView>
  );
}
