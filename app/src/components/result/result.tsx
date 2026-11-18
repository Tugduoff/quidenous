import { type Round, setRoundPinned, roundKeys, useRound } from '@/lib/api/rounds';
import { useTheme } from '@/lib/theme';
import { useT } from '@/lib/i18n';
import { useQueryClient, useMutation } from '@tanstack/react-query';
import { historyKeys } from '@/lib/api/history';
import { View, type ScrollView, Text } from 'react-native';
import { BackButton } from '@/components/ui/back-button';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Pin, ArrowRight, ChevronsRight } from 'lucide-react-native';
import type { Member } from '@/lib/api/group';
import { type ReactNode, useRef, type ComponentRef, useState } from 'react';
import { useSession } from '@/lib/session';
import { Button } from '@/components/ui/button';
import { Screen } from '@/components/ui/screen';
import { QueryState } from '@/components/ui/query-state';
import Animated from 'react-native-reanimated';
import { motion } from '@/lib/motion';
import { ResultSummary, RankResult, ResultBars } from '@/components/result/result-bars';
import { ChatInput, ChatMessages } from '@/components/chat/chat';
import { SaidResultView } from '@/components/said/said';
import { ChooseResult } from '@/components/choose/choose';
import { LiveReactions } from '@/components/result/live-reactions';
import { TripotResult } from '@/components/tripot/tripot';
import { Challenges } from '@/components/community/challenge';
import { Question } from '@/components/vote/question';
import { Header } from '@/components/vote/header';

/** Back arrow (and pin) that replace the header when a result is opened from the history. */
function BackHeader({ onBack, round }: { onBack: () => void; round?: Round }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const pin = useMutation({
    mutationFn: (pinned: boolean) => setRoundPinned(round!.id, pinned),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: roundKeys.detail(round!.id) });
      qc.invalidateQueries({ queryKey: historyKeys.pinned });
    },
  });
  // Show the state being asked for right away; the server echo confirms it.
  const pinned = pin.isPending ? pin.variables : round?.pinned;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
      <BackButton onPress={onBack} />
      {round && (
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr(pinned ? 'vote.unpin' : 'vote.pin')}
          onPress={() => pin.mutate(!pinned)}
          disabled={pin.isPending}
          hitSlop={8}
          style={{
            width: 40,
            height: 40,
            borderRadius: 20,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: pinned ? t.accSoft : t.card,
          }}>
          <Pin size={18} color={pinned ? t.accInk : t.txt} fill={pinned ? t.accInk : 'none'} />
        </PressableScale>
      )}
    </View>
  );
}

export function Result({
  roundId,
  byId,
  onNext,
  onBack,
  gate,
  skip,
}: {
  roundId: number;
  byId: Map<number, Member>;
  onNext?: () => void;
  onBack?: () => void;
  /** Shown instead of "Suivante" while a paced room waits for its next question. */
  gate?: ReactNode;
  /** Several results waiting: "Tout passer" next to "Suivante" marks them all seen. */
  skip?: { count: number; onPress: () => void };
}) {
  const t = useTheme();
  const tr = useT();
  const roundQuery = useRound(roundId);
  const { data: round } = roundQuery;
  const { me } = useSession();
  const scroll = useRef<ComponentRef<typeof ScrollView>>(null);
  // One scroll for the whole page (no box inside a box). Where the full result ends, and whether
  // it has scrolled out of sight: then its one-line summary stays pinned on top.
  const resultEnd = useRef(0);
  const [away, setAway] = useState(false);
  // Follow the newest message while the bottom is in view, and right after I write or send one.
  const follow = useRef(false);
  const toEnd = () => {
    follow.current = true;
    // After the keyboard has opened (and the page shrunk), not before.
    setTimeout(() => scroll.current?.scrollToEnd({ animated: true }), 300);
  };
  const header = onBack ? <BackHeader onBack={onBack} round={round ?? undefined} /> : <Header />;
  const nextButton = onNext && (
    <Button onPress={onNext} style={skip && { flex: 1 }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('vote.next')}</Text>
      <ArrowRight size={20} color={t.accTxt} />
    </Button>
  );
  const next =
    gate ??
    (skip && nextButton ? (
      <View style={{ flexDirection: 'row', gap: 10 }}>
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('vote.skipAll.a11y', { count: skip.count })}
          onPress={skip.onPress}
          style={{
            height: 58,
            borderRadius: 999,
            paddingHorizontal: 18,
            flexDirection: 'row',
            alignItems: 'center',
            gap: 6,
            backgroundColor: t.card2,
          }}>
          <ChevronsRight size={18} color={t.txt} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('vote.skipAll.label')}</Text>
          <Text style={{ fontFamily: t.font.display, fontSize: 15, color: t.mut, fontVariant: ['tabular-nums'] }}>{skip.count}</Text>
        </PressableScale>
        {nextButton}
      </View>
    ) : (
      nextButton
    ));
  // Not loaded (or failed): "Suivante" stays, so an unseen result never blocks the next question.
  if (!round?.result && !round?.choose && !round?.said?.result)
    return (
      <Screen tabs={!onBack} footer={roundQuery.isLoading ? undefined : next}>
        {header}
        <QueryState query={roundQuery} />
      </Screen>
    );
  return (
    <Screen
      // Opened from the history, the screen has no tab bar to keep it clear of the iPhone home bar.
      tabs={!onBack}
      scroller={scroll}
      onScroll={({ contentOffset, layoutMeasurement, contentSize }) => {
        setAway(resultEnd.current > 0 && contentOffset.y > resultEnd.current - 24);
        follow.current = contentOffset.y + layoutMeasurement.height >= contentSize.height - 80;
      }}
      onContentSizeChange={() => follow.current && scroll.current?.scrollToEnd({ animated: true })}
      overlay={
        away && (
          <Animated.View entering={motion.drop} exiting={motion.exit} style={{ position: 'absolute', top: 8, left: 20, right: 20 }}>
            <ResultSummary
              result={round.result ?? []}
              ranking={round.ranking}
              choose={round.choose}
              said={round.kind === 'said' ? round.said?.result : undefined}
              byId={byId}
              onPress={() => {
                follow.current = false;
                scroll.current?.scrollTo({ y: 0, animated: true });
              }}
            />
          </Animated.View>
        )
      }
      footer={
        <>
          <ChatInput roundId={round.id} onFocus={toEnd} onSent={toEnd} />
          {next}
        </>
      }>
      {header}
      <View
        style={{ gap: 16 }}
        onLayout={({ nativeEvent: { layout } }) => {
          resultEnd.current = layout.y + layout.height;
        }}>
        <Question round={round} result />
        {round.kind === 'said' && round.said?.result ? (
          <SaidResultView roundId={round.id} said={round.said} byId={byId} me={me?.id} />
        ) : round.kind === 'choose' && round.choose ? (
          <ChooseResult round={round} data={round.choose} byId={byId} />
        ) : round.ranking ? (
          <RankResult ranking={round.ranking} byId={byId} mine={round.my_ranking} />
        ) : (
          <ResultBars result={round.result ?? []} byId={byId} anonymous={round.anonymous} />
        )}
      </View>
      <LiveReactions roundId={round.id} byId={byId} />
      <TripotResult round={round} byId={byId} />
      <Challenges roundId={round.id} />
      <ChatMessages roundId={round.id} />
    </Screen>
  );
}
