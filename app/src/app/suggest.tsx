import { useIsMutating } from '@tanstack/react-query';
import { router } from 'expo-router';
import { ArrowUp, ChevronLeft, CircleCheck, CircleX, Hourglass, SkipForward, ThumbsDown, ThumbsUp, Trophy } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import { GestureDetector, GestureHandlerRootView, usePanGesture } from 'react-native-gesture-handler';
import Animated, { interpolate, useAnimatedStyle, useReducedMotion, useSharedValue, withSpring, withTiming } from 'react-native-reanimated';
import { scheduleOnRN } from 'react-native-worklets';

import { Screen } from '@/components/screen';
import { Card, Display, PressableScale, QuestionText, Toggle } from '@/components/ui';
import {
  communityError,
  type Suggestion,
  SUGGESTION_MAX,
  SUGGESTION_MIN,
  SUGGESTIONS_PER_DAY,
  type SuggestionVote,
  useMySuggestions,
  useNextSuggestions,
  useSuggest,
  useVoteSuggestion,
  VOTE_KEY,
} from '@/lib/community';
import { useT } from '@/lib/i18n';
import { EASE, motion, SPRING } from '@/lib/motion';
import { sendOnEnter } from '@/lib/submit';
import { useTheme } from '@/lib/theme';

/** "Proposer": write a question for the official deck, vote on others' suggestions, follow mine. */
export default function SuggestScreen() {
  const t = useTheme();
  const tr = useT();
  const mine = useMySuggestions();
  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('common.back')}
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/questions'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>{tr('community.title')}</Display>
      <Compose left={mine.data?.left} />
      <VoteStack />
      <Mine items={mine.data?.items ?? []} />
    </Screen>
  );
}

function Compose({ left }: { left: number | undefined }) {
  const t = useTheme();
  const tr = useT();
  const [text, setText] = useState('');
  const [rank, setRank] = useState(false);
  const [note, setNote] = useState<{ text: string; error: boolean } | null>(null);
  const suggest = useSuggest();
  const out = left === 0;
  const ok = text.trim().length >= SUGGESTION_MIN && !out && !suggest.isPending;
  const send = () => {
    if (!ok) return;
    setNote(null);
    suggest.mutate(
      { text: text.trim(), kind: rank ? 'rank' : 'pick' },
      {
        onSuccess: () => {
          setText('');
          setRank(false);
          setNote({ text: tr('community.compose.sent'), error: false });
        },
        onError: (e) => setNote({ text: communityError(e), error: true }),
      },
    );
  };
  return (
    <View style={{ gap: 8 }}>
      <Card style={{ paddingBottom: 10 }}>
        <TextInput
          accessibilityLabel={tr('community.compose.label')}
          placeholder={rank ? tr('community.compose.placeholderRank') : tr('community.compose.placeholder')}
          placeholderTextColor={t.mut}
          value={text}
          onChangeText={(v) => {
            setText(v);
            if (note) setNote(null);
          }}
          multiline
          maxLength={SUGGESTION_MAX}
          {...sendOnEnter(send)}
          style={{ minHeight: 76, padding: 16, paddingBottom: 6, fontFamily: t.font.regular, fontSize: 16, color: t.txt, textAlignVertical: 'top' }}
        />
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingLeft: 16, paddingRight: 10 }}>
          <Toggle icon={Trophy} label={tr('community.compose.ranking')} value={rank} onChange={setRank} />
          <View style={{ flex: 1 }} />
          {left != null && (
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 13, color: out ? t.tag : t.mut }}>
              {tr('community.compose.left', { left, max: SUGGESTIONS_PER_DAY })}
            </Text>
          )}
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('community.compose.send')}
            disabled={!ok}
            onPress={send}
            style={{
              width: 44,
              height: 44,
              borderRadius: 22,
              backgroundColor: ok ? t.acc : t.card2,
              alignItems: 'center',
              justifyContent: 'center',
            }}>
            <ArrowUp size={22} color={ok ? t.accTxt : t.mut} />
          </PressableScale>
        </View>
      </Card>
      {note && (
        <Animated.Text
          entering={motion.enter}
          style={{ marginLeft: 4, fontFamily: t.font.semibold, fontSize: 14, color: note.error ? t.tag : t.accInk }}>
          {note.text}
        </Animated.Text>
      )}
    </View>
  );
}

/** Height of the card stack; the card behind peeks out under the top one. */
const STACK = 210;
const PEEK = 10;
/** How far a card must be dragged (or flung) to count as a vote. */
const SWIPE = 110;

/** Others' suggestions, one card at a time: swipe right (up) or left (down), or the buttons below. */
function VoteStack() {
  const t = useTheme();
  const tr = useT();
  const { data, isFetching, refetch } = useNextSuggestions();
  const vote = useVoteSuggestion();
  const voting = useIsMutating({ mutationKey: VOTE_KEY });
  const [gone, setGone] = useState<Set<number>>(() => new Set());
  const [error, setError] = useState<string | null>(null);
  // A button pressed: the top card flies off that way first, then the vote is sent.
  const [fly, setFly] = useState<{ id: number; value: SuggestionVote } | null>(null);
  const queue = (data ?? []).filter((s) => !gone.has(s.id));
  const top = queue[0];

  // All voted: ask for more once the votes are in (the server leaves out what I voted on).
  useEffect(() => {
    if (!data?.length || queue.length || voting || isFetching) return;
    refetch().then(() => setGone(new Set()));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data?.length, queue.length, voting, isFetching]);

  const decide = (id: number, value: SuggestionVote) => {
    setFly(null);
    setError(null);
    setGone((g) => new Set(g).add(id));
    vote.mutate({ id, value }, { onError: (e) => setError(communityError(e)) });
  };

  const label = { fontFamily: t.font.bold, fontSize: 18, color: t.txt } as const;
  if (!data) return null;
  if (!top)
    return (
      <View style={{ gap: 10, marginTop: 6 }}>
        <Text style={label}>{tr('community.vote.title')}</Text>
        {!isFetching && <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('community.vote.empty')}</Text>}
      </View>
    );
  const press = (value: SuggestionVote) => !fly && setFly({ id: top.id, value });
  return (
    <View style={{ gap: 14, marginTop: 6 }}>
      <Text style={label}>{tr('community.vote.title')}</Text>
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
      {error && <Text style={{ alignSelf: 'center', fontFamily: t.font.semibold, fontSize: 14, color: t.tag }}>{error}</Text>}
    </View>
  );
}

function RoundButton({
  label,
  size,
  onPress,
  accent,
  soft,
  children,
}: {
  label: string;
  size: number;
  onPress: () => void;
  accent?: boolean;
  soft?: boolean;
  children: React.ReactNode;
}) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      onPress={onPress}
      hitSlop={6}
      style={[
        {
          width: size,
          height: size,
          borderRadius: size / 2,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: accent ? t.acc : soft ? t.card2 : t.card,
        },
        !soft && t.shadow,
      ]}>
      {children}
    </PressableScale>
  );
}

/** One suggestion card. Only the top one can be dragged; the one behind sits a little smaller and lower. */
function SwipeCard({ s, top, fly, onDecide }: { s: Suggestion; top: boolean; fly: SuggestionVote | null; onDecide: (v: SuggestionVote) => void }) {
  const t = useTheme();
  const reduced = useReducedMotion();
  const tx = useSharedValue(0);
  const ty = useSharedValue(0);
  const depth = useSharedValue(top ? 0 : 1);
  useEffect(() => {
    depth.value = withSpring(top ? 0 : 1, SPRING);
  }, [top, depth]);

  // Off the screen to the right (up), left (down) or upwards (skipped), then the vote.
  const away = (v: SuggestionVote) => {
    if (reduced) return onDecide(v);
    const done = (finished?: boolean) => {
      'worklet';
      if (finished) scheduleOnRN(onDecide, v);
    };
    const cfg = { duration: 220, easing: EASE };
    if (v === 0) ty.value = withTiming(-STACK * 1.6, cfg, done);
    else tx.value = withTiming(v * 520, cfg, done);
  };
  useEffect(() => {
    if (fly != null) away(fly);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fly]);

  const pan = usePanGesture({
    enabled: top,
    // Sideways only: dragging up and down still scrolls the page.
    activeOffsetX: [-12, 12],
    failOffsetY: [-14, 14],
    onUpdate: (e) => {
      'worklet';
      tx.value = e.translationX;
    },
    onDeactivate: (e) => {
      'worklet';
      const x = e.translationX;
      const flung = Math.abs(x) > 40 && Math.abs(e.velocityX) > 800 && Math.sign(x) === Math.sign(e.velocityX);
      if (Math.abs(x) > SWIPE || flung) {
        const v: SuggestionVote = x > 0 ? 1 : -1;
        tx.value = withTiming(v * 520, { duration: 180, easing: EASE }, (finished) => {
          'worklet';
          if (finished) scheduleOnRN(onDecide, v);
        });
      } else tx.value = withSpring(0, SPRING);
    },
  });

  const card = useAnimatedStyle(() => ({
    transform: [
      { translateX: tx.value },
      { translateY: ty.value + depth.value * PEEK },
      { rotate: `${tx.value / 22}deg` },
      { scale: 1 - depth.value * 0.05 },
    ],
    opacity: 1 - depth.value * 0.4,
  }));
  const upHint = useAnimatedStyle(() => ({ opacity: interpolate(tx.value, [20, SWIPE], [0, 1], 'clamp') }));
  const downHint = useAnimatedStyle(() => ({ opacity: interpolate(tx.value, [-SWIPE, -20], [1, 0], 'clamp') }));
  const long = s.text.length > 90;
  const hint = { position: 'absolute', top: 14, width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center' } as const;

  return (
    <GestureDetector gesture={pan}>
      <Animated.View
        entering={top ? undefined : motion.enter}
        style={[
          {
            position: 'absolute',
            left: 0,
            right: 0,
            top: 0,
            height: STACK,
            borderRadius: t.radius.card,
            backgroundColor: t.card,
            padding: 22,
            justifyContent: 'center',
          },
          t.shadow,
          card,
        ]}>
        <QuestionText
          text={s.text}
          rank={s.kind === 'rank'}
          style={{ fontFamily: t.font.display, fontSize: long ? 19 : 23, lineHeight: long ? 23 : 27, letterSpacing: -0.4, color: t.txt }}
        />
        <Animated.View style={[hint, { right: 14, backgroundColor: t.acc }, upHint]}>
          <ThumbsUp size={18} color={t.accTxt} />
        </Animated.View>
        <Animated.View style={[hint, { left: 14, backgroundColor: t.card2 }, downHint]}>
          <ThumbsDown size={18} color={t.txt} />
        </Animated.View>
      </Animated.View>
    </GestureDetector>
  );
}

/** My suggestions and where each one stands; the three newest unless opened. */
function Mine({ items }: { items: Suggestion[] }) {
  const t = useTheme();
  const tr = useT();
  const [all, setAll] = useState(false);
  if (!items.length) return null;
  const shown = all ? items : items.slice(0, 3);
  return (
    <View style={{ gap: 10, marginTop: 6 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('community.mine.title')}</Text>
        {items.length > 3 && (
          <Pressable accessibilityRole="button" onPress={() => setAll(!all)} hitSlop={8}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>
              {tr(all ? 'community.mine.less' : 'community.mine.more')}
            </Text>
          </Pressable>
        )}
      </View>
      {shown.map((s) => (
        <Animated.View key={s.id} entering={motion.enter} layout={motion.layout}>
          <Card style={{ padding: 14, gap: 6 }}>
            <QuestionText
              text={s.text}
              rank={s.kind === 'rank'}
              style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}
            />
            <Status s={s} />
          </Card>
        </Animated.View>
      ))}
    </View>
  );
}

function Status({ s }: { s: Suggestion }) {
  const t = useTheme();
  const tr = useT();
  const score = s.up - s.down;
  const [Icon, text, color] =
    s.status === 'approved'
      ? [CircleCheck, tr('community.mine.approved'), t.accInk]
      : s.status === 'rejected'
        ? [CircleX, tr('community.mine.rejected'), t.mut]
        : s.status === 'queued'
          ? [Hourglass, tr('community.mine.queued'), t.mut]
          : [ThumbsUp, tr('community.mine.voting', { score: score > 0 ? `+${score}` : String(score) }), t.mut];
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
      <Icon size={14} color={color} />
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 13, color }}>
        {text}
      </Text>
    </View>
  );
}
