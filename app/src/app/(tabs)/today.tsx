// « Aujourd'hui »: every room of mine on one screen, what each one wants from me today, most pressing first
// (GET /api/today, lib/today.ts). A tab only with two rooms or more; tapping a card opens that room's vote tab.
import { useQueryClient } from '@tanstack/react-query';
import { router, useIsFocused } from 'expo-router';
import { ArrowRight, ChevronRight, Coins, Flame, MessageCircle } from 'lucide-react-native';
import { memo, useCallback, useEffect, useMemo, useState } from 'react';
import { Pressable, RefreshControl, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withRepeat, withTiming } from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { dateFormat, useLang, useT } from '@/lib/i18n';
import { EASE, motion, useArrivals } from '@/lib/motion';
import { useSwitchRoom } from '@/lib/hooks/use-switch-room';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { until } from '@/lib/utils/time';
import { type TodayRoom, useToday, todayKeys } from '@/lib/api/today';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { TAG_BG } from '@/components/ui/tag';
import { Card } from '@/components/ui/card';
import { Display, Logo, QuestionText } from '@/components/ui/text';
import { HOT_FROM } from '@/lib/utils/streaks';
import { cardQuestion, missing, needsMe, streakAtRisk } from '@/lib/utils/today';

export default function TodayScreen() {
  const focused = useIsFocused();
  const today = useToday(focused);
  const rooms = today.data?.rooms;
  const [pulled, setPulled] = useState(false);
  const t = useTheme();
  const refresh = useCallback(() => {
    setPulled(true);
    today.refetch().finally(() => setPulled(false));
  }, [today]);
  const open = useOpenRoom();
  const arrived = useArrivals(rooms?.map((r) => r.id));

  return (
    <Screen refreshControl={<RefreshControl refreshing={pulled} onRefresh={refresh} tintColor={t.mut} colors={[t.accInk]} progressBackgroundColor={t.card} />}>
      <Header />
      {rooms ? (
        <>
          <Summary rooms={rooms} />
          <View style={{ gap: 12 }}>
            {rooms.map((r, i) => (
              <Animated.View key={r.id} layout={motion.layout} entering={arrived(r.id) ? motion.rise : motion.stagger(i)}>
                <TodayCard room={r} onOpen={open} />
              </Animated.View>
            ))}
          </View>
        </>
      ) : today.isError ? (
        <QueryState query={today} />
      ) : (
        <Skeleton />
      )}
    </Screen>
  );
}

/** The logo, today's date, and me (the profile). */
function Header() {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const { lang } = useLang();
  // The day changes under an open app: read at each render, cheap.
  const date = useMemo(() => dateFormat({ weekday: 'long', day: 'numeric', month: 'long' }).format(new Date()), [lang]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
      <View style={{ gap: 4, flexShrink: 1 }}>
        <Logo />
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
          {date.charAt(0).toUpperCase() + date.slice(1)}
        </Text>
      </View>
      <Pressable accessibilityRole="button" accessibilityLabel={tr('vote.header.profile')} onPress={() => router.push('/profile')}>
        <Avatar member={me ?? undefined} size={36} />
      </Pressable>
    </View>
  );
}

/** The big line: how many questions wait for me, else "all caught up" with Roupi, who has nothing to do either. */
const Summary = memo(function Summary({ rooms }: { rooms: TodayRoom[] }) {
  const t = useTheme();
  const tr = useT();
  const votes = rooms.filter((r) => r.state === 'vote').length;
  const results = rooms.filter((r) => r.state === 'result').length;
  const waiting = rooms.some((r) => r.state === 'wait' || r.state === 'tripot');
  const done = votes + results === 0;
  const title = votes ? tr('today.summary.vote', { count: votes }) : results ? tr('today.summary.results', { count: results }) : tr('today.summary.done');
  const sub = votes && results ? tr('today.summary.andResults', { count: results }) : done ? tr(waiting ? 'today.summary.doneWaiting' : 'today.summary.doneIdle') : null;
  return (
    <View accessibilityRole="header" style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 4 }}>
      <View style={{ flex: 1, gap: 6 }}>
        <Display size={30}>{title}</Display>
        {sub && <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{sub}</Text>}
      </View>
      {done && (
        <Animated.View entering={motion.enter}>
          <Mascot who="roupi" mood="sleepy" size={88} />
        </Animated.View>
      )}
    </View>
  );
});

/** Taps a card: that room's vote tab (switching rooms first, its screen fetched on the way). */
function useOpenRoom() {
  const { room } = useSession();
  const switchRoom = useSwitchRoom();
  const qc = useQueryClient();
  return useCallback(
    (id: number) => {
      if (id === room) return router.navigate('/');
      // Failed (left that room meanwhile, offline): the cards refresh and tell.
      switchRoom(id, 'room', { warm: true }).catch(() => qc.invalidateQueries({ queryKey: todayKeys.all }));
    },
    // A new function on every render is fine for the hooks above; only the room matters to the cards' memo.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [room],
  );
}

/** One room: its name, my streak and unread chat, the question, and what it wants from me. */
const TodayCard = memo(function TodayCard({ room: r, onOpen }: { room: TodayRoom; onOpen: (id: number) => void }) {
  const t = useTheme();
  const tr = useT();
  const q = cardQuestion(r);
  const mine = needsMe(r);
  const risk = streakAtRisk(r);
  const line = stateLine(r, tr);
  const hint = r.state === 'vote' ? 'today.hint.vote' : r.state === 'result' ? 'today.hint.result' : 'today.hint.other';
  const label = [
    r.name,
    q && q.text,
    line,
    r.streak && r.streak.days > 0 ? tr('streaks.a11y', { count: r.streak.days }) : null,
    risk ? tr('today.atRisk') : null,
    r.unread ? tr('today.unreadA11y', { count: r.unread }) : null,
  ]
    .filter(Boolean)
    .join('. ');
  return (
    <PressableScale scale={0.98} accessibilityRole="button" accessibilityLabel={label} accessibilityHint={tr(hint)} onPress={() => onOpen(r.id)}>
      <Card style={{ padding: 16, gap: 10, borderWidth: 1.5, borderColor: r.state === 'vote' ? t.accInk : r.state === 'result' ? t.tagInk : 'transparent' }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
            {r.name}
          </Text>
          {r.streak && r.streak.days > 0 && <StreakChip days={r.streak.days} risk={risk} />}
          {r.unread > 0 && <UnreadChip count={r.unread} />}
        </View>
        {q ? (
          <QuestionText
            text={q.text}
            rank={q.kind === 'rank'}
            choose={q.kind === 'choose'}
            style={{ fontFamily: t.font.bold, fontSize: 18, lineHeight: 24, color: r.state === 'idle' ? t.mut : t.txt }}
          />
        ) : null}
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginTop: 2 }}>
          <StatePill room={r} line={line} />
          <View style={{ flex: 1 }} />
          {r.state === 'vote' || r.state === 'wait' || r.state === 'tripot' ? <Votes votes={r.votes} needed={r.needed} /> : null}
          {!mine && <ChevronRight size={18} color={t.mut} />}
        </View>
      </Card>
    </PressableScale>
  );
});

function stateLine(r: TodayRoom, tr: ReturnType<typeof useT>) {
  switch (r.state) {
    case 'vote':
      return tr('today.state.vote');
    case 'result':
      return tr('today.state.result', { count: Math.max(1, r.unseen) });
    case 'tripot':
      return tr('today.state.tripot');
    case 'wait': {
      const n = missing(r);
      return n ? tr('today.state.wait', { count: n }) : tr('today.state.waitAll');
    }
    default:
      return r.next_open_at ? tr('today.state.next', { time: until(r.next_open_at) }) : tr('today.state.none');
  }
}

/** What the room wants: a loud pill when it's me (vote: the accent, a result: pink), a quiet line otherwise. */
function StatePill({ room: r, line }: { room: TodayRoom; line: string }) {
  const t = useTheme();
  if (r.state === 'vote' || r.state === 'result') {
    const vote = r.state === 'vote';
    const ink = vote ? t.accTxt : '#fff';
    return (
      <View
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 6,
          height: 32,
          paddingHorizontal: 14,
          borderRadius: t.radius.pill,
          backgroundColor: vote ? t.acc : TAG_BG,
        }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: ink }}>{line}</Text>
        <ArrowRight size={16} color={ink} strokeWidth={2.5} />
      </View>
    );
  }
  const tripot = r.state === 'tripot';
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, flexShrink: 1 }}>
      {tripot && <Coins size={15} color={t.accInk} />}
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: tripot ? t.accInk : t.mut }}>
        {line}
      </Text>
    </View>
  );
}

/** My streak here; pink when today has no vote yet and a question waits (it ends tonight otherwise). */
function StreakChip({ days, risk }: { days: number; risk: boolean }) {
  const t = useTheme();
  const tr = useT();
  const color = risk ? t.tagInk : days >= HOT_FROM ? t.accInk : t.mut;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 3 }}>
      <Flame size={14} color={color} strokeWidth={2.5} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color }}>{risk ? `${days} · ${tr('today.atRisk')}` : days}</Text>
    </View>
  );
}

function UnreadChip({ count }: { count: number }) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4, height: 22, paddingHorizontal: 8, borderRadius: 11, backgroundColor: t.card2 }}>
      <MessageCircle size={13} color={t.txt} strokeWidth={2.5} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 12, color: t.txt }}>{count > 99 ? '99+' : count}</Text>
    </View>
  );
}

/** Votes in out of needed: a short bar and the count. */
function Votes({ votes, needed }: { votes: number; needed: number }) {
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

/** While the first answer loads: the shape of the screen, breathing (still under reduced motion). */
function Skeleton() {
  const t = useTheme();
  const reduced = useReducedMotion();
  const o = useSharedValue(1);
  useEffect(() => {
    if (!reduced) o.value = withRepeat(withTiming(0.55, { duration: 700, easing: EASE }), -1, true);
  }, [o, reduced]);
  const breathe = useAnimatedStyle(() => ({ opacity: o.value }));
  const bar = (w: number | `${number}%`, h: number, bg = t.card2) => <View style={{ width: w, height: h, borderRadius: h / 2, backgroundColor: bg }} />;
  return (
    <Animated.View style={[{ gap: 16 }, breathe]} accessibilityElementsHidden importantForAccessibility="no-hide-descendants">
      <View style={{ gap: 10, marginTop: 4 }}>
        {bar('70%', 30, t.card)}
        {bar('45%', 14, t.card)}
      </View>
      {[0, 1, 2].map((i) => (
        <Card key={i} style={{ padding: 16, gap: 12 }}>
          {bar('35%', 12)}
          {bar('90%', 16)}
          {bar('60%', 16)}
          {bar(120, 30)}
        </Card>
      ))}
    </Animated.View>
  );
}
