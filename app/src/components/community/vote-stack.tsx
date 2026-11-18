import { useIsMutating } from '@tanstack/react-query';
import { Flag, SkipForward, ThumbsDown, ThumbsUp } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { QueryState } from '@/components/ui/query-state';
import { communityError, type SuggestionVote, useNextSuggestions, useVoteSuggestion, VOTE_KEY } from '@/lib/api/community';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { useFeatures } from '@/lib/api/features';
import { useWhen } from '@/lib/hooks/use-on-change';
import { PEEK, STACK, SwipeCard } from '@/components/community/swipe-card';
import { RoundButton } from '@/components/community/round-button';

/** Others' suggestions, one card at a time: swipe right (up) or left (down), or the buttons below. */
export function VoteStack() {
  const t = useTheme();
  const tr = useT();
  const next = useNextSuggestions();
  const { data, isFetching, refetch } = next;
  const vote = useVoteSuggestion();
  const voting = useIsMutating({ mutationKey: VOTE_KEY });
  const [gone, setGone] = useState<Set<number>>(() => new Set());
  const [error, setError] = useState<string | null>(null);
  // A button pressed: the top card flies off that way first, then the vote is sent.
  const [fly, setFly] = useState<{ id: number; value: SuggestionVote } | null>(null);
  // Reporting the card on top (a server that takes suggestion reports).
  const { moderation } = useFeatures();
  const [toReport, setToReport] = useState<ModTarget | null>(null);
  const queue = (data ?? []).filter((s) => !gone.has(s.id));
  const top = queue[0];

  // All voted: ask for more once the votes are in (the server leaves out what I voted on).
  useWhen(!!data?.length && !queue.length && !voting && !isFetching, () => {
    refetch().then(() => setGone(new Set()));
  });

  const decide = (id: number, value: SuggestionVote) => {
    setFly(null);
    setError(null);
    setGone((g) => new Set(g).add(id));
    vote.mutate({ id, value }, { onError: (e) => setError(communityError(e)) });
  };

  const label = { fontFamily: t.font.bold, fontSize: 18, color: t.txt } as const;
  if (!data) return <QueryState query={next} />;
  if (!top) return null;
  const press = (value: SuggestionVote) => !fly && setFly({ id: top.id, value });
  return (
    <View style={{ gap: 14, marginTop: 6 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={label}>{tr('community.vote.title')}</Text>
        {moderation && (
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={tr('moderation.reportQuestion')}
            hitSlop={8}
            onPress={() => setToReport({ kind: 'suggestion', id: top.id })}
            style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center' }}>
            <Flag size={17} color={t.mut} />
          </Pressable>
        )}
      </View>
      {/* Reported: the card goes. */}
      <ModerationDialog target={toReport} onClose={() => setToReport(null)} onReported={(r) => setGone((g) => new Set(g).add(r.id))} />
      <GestureHandlerRootView style={{ height: STACK + PEEK }}>
        {/* The card behind first, so the top one is drawn over it. */}
        {queue
          .slice(0, 2)
          .reverse()
          .map((s) => (
            <SwipeCard key={s.id} s={s} top={s.id === top.id} fly={fly?.id === s.id ? fly.value : null} onDecide={(v) => decide(s.id, v)} />
          ))}
      </GestureHandlerRootView>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 22 }}>
        <RoundButton label={tr('community.vote.down')} size={56} onPress={() => press(-1)}>
          <ThumbsDown size={24} color={t.txt} />
        </RoundButton>
        <RoundButton label={tr('community.vote.skip')} size={44} onPress={() => press(0)} soft>
          <SkipForward size={18} color={t.mut} />
        </RoundButton>
        <RoundButton label={tr('community.vote.up')} size={56} onPress={() => press(1)} accent>
          <ThumbsUp size={24} color={t.accTxt} />
        </RoundButton>
      </View>
      {error && <Text style={{ alignSelf: 'center', fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk }}>{error}</Text>}
    </View>
  );
}
