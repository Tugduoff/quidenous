// « Qui a dit ça ? » (docs/SAID.md): everyone writes an answer, then matches the answers to their authors, then the reveal.
//
// The pieces (AnswerField, MyAnswer, MatchBoard + SuspectTray, GuessRecap, SaidReveal, SaidAnswerList, SaidScoreboard,
// SaidResultView) only take props: no room query in them, so a party screen can reuse them. `SaidRound` is the room's
// open question (it talks to `/round/current/*`).
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowRight, Check, EyeOff, Pencil, Quote, RotateCcw, Trophy, X } from 'lucide-react-native';
import { memo, type ReactNode, useCallback, useEffect, useMemo, useReducer, useState, useSyncExternalStore } from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import Animated, { FadeIn, FadeInDown, ReduceMotion, useAnimatedStyle, useReducedMotion, useSharedValue, withDelay, withRepeat, withSequence, withSpring, withTiming } from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { Screen } from '@/components/ui/screen';
import { Tip } from '@/components/ui/tip';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { BOUNCE, EASE, LAYOUT_EASE, motion, usePop } from '@/lib/motion';
import { ANSWER_MAX, ANSWER_WARN, guessGroups, initMatch, isComplete, type Match, type MatchAction, matchReducer, pickLine, placed, type RevealItem, revealItems, suspects as suspectIds, toGuess, toGuesses, places } from '@/lib/utils/said';
import { storage } from '@/lib/device/storage';
import { sendOnEnter } from '@/components/ui/send-on-enter';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar, AvatarStack } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import type { Round, SaidAnswer, SaidGuess, SaidResult, SaidView } from '@/lib/api/rounds';
import { roundKeys, answerCurrent, voteCurrent } from '@/lib/api/rounds';
import { groupKeys } from '@/lib/api/group';
import { impactHaptic, selectionHaptic } from '@/lib/device/haptics';

const sys = ReduceMotion.System;

const tap = () => {
  selectionHaptic();
};
const thud = () => {
  impactHaptic();
};

/** "Léa Martin" → "Léa": chips and signatures stay short. */
const firstName = (name: string) => name.trim().split(/\s+/)[0] || name;

/** The small capitals over a block ("TA RÉPONSE"). `k`: a size factor (the big screen). */
function Label({ children, k = 1 }: { children: string; k?: number }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.bold, fontSize: Math.round(11 * k), letterSpacing: 0.6 * k, color: t.mut }}>{children}</Text>;
}

/** An answer's text with its opening quote mark, or "Réponse masquée" (hidden by reports). */
export function AnswerText({ answer, size = 17 }: { answer: Pick<SaidAnswer, 'text' | 'hidden'>; size?: number }) {
  const t = useTheme();
  const tr = useT();
  if (answer.text == null)
    return (
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, minHeight: size * 1.35 }}>
        <EyeOff size={size - 1} color={t.mut} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: size - 2, color: t.mut, fontStyle: 'italic' }}>{tr('said.guess.hidden')}</Text>
      </View>
    );
  return <Text style={{ fontFamily: t.font.semibold, fontSize: size, lineHeight: Math.round(size * 1.35), color: t.txt }}>{answer.text}</Text>;
}

/** The answer's number in a disc: the same number on its card, in the tray and in the recap. */
export function NumberDisc({ n, on, size = 26 }: { n: number; on?: boolean; size?: number }) {
  const t = useTheme();
  return (
    <View
      style={{
        width: size,
        height: size,
        borderRadius: size / 2,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: on ? t.acc : t.card2,
      }}>
      <Text style={{ fontFamily: t.font.display, fontSize: Math.round(size / 2), color: on ? t.accTxt : t.mut }}>{n}</Text>
    </View>
  );
}

// ---------------------------------------------------------------------------------------------------------------------
// Writing

/** The answer being written: a big field, the counter near the limit, Enter sends. Controlled (the button is elsewhere). */
export function AnswerField({
  value,
  onChange,
  onSubmit,
  autoFocus,
  error,
}: {
  value: string;
  onChange: (text: string) => void;
  onSubmit: () => void;
  autoFocus?: boolean;
  /** Under the field, in pink (not sent, too late). */
  error?: string | null;
}) {
  const t = useTheme();
  const tr = useT();
  const left = ANSWER_MAX - [...value].length;
  const near = ANSWER_MAX - left >= ANSWER_WARN;
  return (
    <Card style={{ paddingTop: 4, paddingBottom: 12 }}>
      <TextInput
        accessibilityLabel={tr('said.write.label')}
        placeholder={tr('said.write.placeholder')}
        placeholderTextColor={t.mut}
        value={value}
        onChangeText={onChange}
        autoFocus={autoFocus}
        multiline
        maxLength={ANSWER_MAX}
        {...sendOnEnter(onSubmit)}
        style={{
          minHeight: 92,
          paddingHorizontal: 16,
          paddingTop: 12,
          paddingBottom: 6,
          fontFamily: t.font.semibold,
          fontSize: 18,
          lineHeight: 24,
          color: t.txt,
          textAlignVertical: 'top',
        }}
      />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, paddingHorizontal: 16 }}>
        <EyeOff size={13} color={t.mut} />
        <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>{tr('said.write.hint')}</Text>
        {near && (
          <Text
            accessibilityLiveRegion="polite"
            accessibilityLabel={tr('said.write.left', { count: left })}
            style={{ fontFamily: t.font.bold, fontSize: 13, color: left <= 10 ? t.tagInk : t.mut, fontVariant: ['tabular-nums'] }}>
            {left}
          </Text>
        )}
      </View>
      {!!error && (
        <Text accessibilityRole="alert" style={{ paddingHorizontal: 16, paddingTop: 8, fontFamily: t.font.semibold, fontSize: 13, color: t.tagInk }}>
          {error}
        </Text>
      )}
    </Card>
  );
}

/** My answer once sent: anonymous for the others until the guessing, rewritable until then (`onEdit`). */
export function MyAnswer({ text, onEdit }: { text: string; onEdit?: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Card style={{ padding: 16, gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <Quote size={14} color={t.accInk} fill={t.accInk} />
        <View style={{ flex: 1 }}>
          <Label>{tr('said.write.yours')}</Label>
        </View>
        {onEdit && (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('said.write.editA11y')}
            onPress={onEdit}
            hitSlop={6}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 36,
              paddingHorizontal: 14,
              borderRadius: t.radius.pill,
              backgroundColor: t.card2,
            }}>
            <Pencil size={14} color={t.txt} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{tr('said.write.edit')}</Text>
          </PressableScale>
        )}
      </View>
      <AnswerText answer={{ text, hidden: false }} size={19} />
    </Card>
  );
}

/** Under the field: "Envoyer", or "Annuler" and "Enregistrer" while rewriting. */
export function AnswerButtons({
  editing,
  ok,
  busy,
  onSubmit,
  onCancel,
}: {
  editing: boolean;
  ok: boolean;
  busy: boolean;
  onSubmit: () => void;
  onCancel: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  if (!editing)
    return (
      <Button disabled={!ok} loading={busy} onPress={onSubmit}>
        {tr('said.write.send')}
      </Button>
    );
  return (
    <View style={{ flexDirection: 'row', gap: 10 }}>
      <PressableScale
        accessibilityRole="button"
        onPress={onCancel}
        style={{
          height: 58,
          paddingHorizontal: 22,
          borderRadius: t.radius.pill,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: t.card2,
        }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{tr('said.write.cancel')}</Text>
      </PressableScale>
      <Button style={{ flex: 1 }} disabled={!ok} loading={busy} onPress={onSubmit}>
        {tr('said.write.save')}
      </Button>
    </View>
  );
}

// ---------------------------------------------------------------------------------------------------------------------
// Guessing: the matching game

/**
 * The game's state for these answers (ids in the order shown). The answers are frozen once the guessing starts, so the
 * order never changes under a game; a screen that could get another set keys its board by the order.
 */
export function useMatchGame(order: number[]) {
  return useReducer(matchReducer, order, (o) => initMatch(o));
}

/** Every answer to place, as cards: tap one, then a person in the tray. Long-press (or the screen reader's action) reports. */
export function MatchBoard({
  answers,
  state,
  byId,
  dispatch,
  onReport,
}: {
  /** In the order shown (`state.order`). */
  answers: SaidAnswer[];
  state: Match;
  byId: Map<number, Member>;
  dispatch: (a: MatchAction) => void;
  /** Long-press on an answer: report it (not on a hidden one). */
  onReport?: (answerId: number) => void;
}) {
  const onSelect = useCallback(
    (id: number) => {
      tap();
      dispatch({ type: 'select', answer: id });
    },
    [dispatch],
  );
  const onClear = useCallback((id: number) => dispatch({ type: 'clear', answer: id }), [dispatch]);
  return (
    <View style={{ gap: 12 }}>
      {answers.map((a, i) => (
        <AnswerCard
          key={a.id}
          answer={a}
          n={i + 1}
          total={answers.length}
          member={state.picks[a.id] != null ? byId.get(state.picks[a.id]) : undefined}
          selected={state.sel === a.id}
          onSelect={onSelect}
          onClear={onClear}
          onReport={onReport}
        />
      ))}
    </View>
  );
}

const AnswerCard = memo(function AnswerCard({
  answer,
  n,
  total,
  member,
  selected,
  onSelect,
  onClear,
  onReport,
}: {
  answer: SaidAnswer;
  n: number;
  total: number;
  member: Member | undefined;
  selected: boolean;
  onSelect: (id: number) => void;
  onClear: (id: number) => void;
  onReport?: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const bump = usePop(member?.id, 1.025, member != null);
  const report = onReport && answer.text != null ? () => onReport(answer.id) : undefined;
  const text = answer.text ?? tr('said.guess.hidden');
  return (
    <Animated.View style={bump}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityState={{ selected }}
        accessibilityLabel={`${tr('said.guess.a11y', { n, total, text })}. ${member ? tr('said.guess.on', { name: member.name }) : tr('said.guess.open')}`}
        accessibilityHint={selected ? tr('said.guess.pickNow') : undefined}
        accessibilityActions={[
          ...(member ? [{ name: 'clear', label: tr('said.guess.clear', { name: member.name }) }] : []),
          ...(report ? [{ name: 'report', label: tr('moderation.reportAnswer') }] : []),
        ]}
        onAccessibilityAction={(e) => {
          if (e.nativeEvent.actionName === 'clear') onClear(answer.id);
          if (e.nativeEvent.actionName === 'report') report?.();
        }}
        onPress={() => onSelect(answer.id)}
        onLongPress={report}
        delayLongPress={450}
        style={[
          {
            padding: 16,
            gap: 14,
            borderRadius: t.radius.card,
            borderWidth: 2,
            borderColor: selected ? t.accInk : 'transparent',
            backgroundColor: selected ? t.accSoft : t.card,
          },
          t.shadow,
        ]}>
        <View style={{ flexDirection: 'row', gap: 12, alignItems: 'flex-start' }}>
          <NumberDisc n={n} on={selected} />
          <View style={{ flex: 1, paddingTop: 1 }}>
            <AnswerText answer={answer} />
          </View>
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'flex-end', gap: 8, minHeight: 36 }}>
          {member ? <Signature member={member} selected={selected} onClear={() => onClear(answer.id)} /> : <WhoSlot waiting={selected} />}
        </View>
      </PressableScale>
    </Animated.View>
  );
});

/** "Qui ?" in a dashed pill; breathes while the card waits for a person. */
function WhoSlot({ waiting }: { waiting: boolean }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const s = useSharedValue(1);
  useEffect(() => {
    s.value =
      waiting && !reduced
        ? withRepeat(withSequence(withTiming(1.07, { duration: 520, easing: EASE }), withTiming(1, { duration: 520, easing: EASE })), -1)
        : withTiming(1, { duration: 120 });
  }, [waiting, reduced, s]);
  const breathe = useAnimatedStyle(() => ({ transform: [{ scale: s.value }] }));
  return (
    <Animated.View
      style={[
        {
          flexDirection: 'row',
          alignItems: 'center',
          gap: 6,
          height: 36,
          paddingLeft: 4,
          paddingRight: 12,
          borderRadius: t.radius.pill,
          borderWidth: 1.5,
          borderStyle: 'dashed',
          borderColor: waiting ? t.accInk : t.mut,
        },
        breathe,
      ]}>
      <View style={{ width: 28, height: 28, borderRadius: 14, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        <Text style={{ fontFamily: t.font.display, fontSize: 15, color: waiting ? t.accInk : t.mut }}>?</Text>
      </View>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: waiting ? t.accInk : t.mut }}>{tr('said.guess.who')}</Text>
    </Animated.View>
  );
}

/** Who I said wrote it, signed at the bottom of the card. On the selected card, an × takes them off. */
function Signature({ member, selected, onClear }: { member: Member; selected: boolean; onClear: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View key={member.id} entering={motion.pop} style={{ flexDirection: 'row', alignItems: 'center', gap: 6, flexShrink: 1 }}>
      <View
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 8,
          height: 36,
          paddingLeft: 4,
          paddingRight: 14,
          borderRadius: t.radius.pill,
          backgroundColor: selected ? t.card : t.card2,
          flexShrink: 1,
        }}>
        <Avatar member={member} size={28} />
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
          {firstName(member.name)}
        </Text>
      </View>
      {selected && (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('said.guess.clear', { name: member.name })}
          onPress={onClear}
          hitSlop={8}
          style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
          <X size={16} color={t.mut} />
        </Pressable>
      )}
    </Animated.View>
  );
}

/**
 * The people to place, pinned over the button: tap one to put them on the waiting answer. Someone already placed shows
 * the answer's number (tapping them moves them, or swaps with whoever was there).
 */
export function SuspectTray({ people, state, dispatch }: { people: Member[]; state: Match; dispatch: (a: MatchAction) => void }) {
  const t = useTheme();
  const tr = useT();
  const onPick = useCallback(
    (id: number) => {
      thud();
      dispatch({ type: 'pick', member: id });
    },
    [dispatch],
  );
  const slotOf = new Map<number, number>();
  state.order.forEach((id, i) => {
    const m = state.picks[id];
    if (m != null) slotOf.set(m, i + 1);
  });
  const waiting = state.sel != null ? state.order.indexOf(state.sel) + 1 : 0;
  const chips = people.map((m) => <SuspectChip key={m.id} member={m} slot={slotOf.get(m.id) ?? 0} onPick={onPick} />);
  // A handful wraps on two lines at most; a crowd scrolls sideways on one.
  const crowd = people.length > 6;
  return (
    <View style={{ gap: 8 }}>
      <Text
        accessibilityLiveRegion="polite"
        numberOfLines={1}
        style={{ textAlign: 'center', fontFamily: t.font.semibold, fontSize: 13, color: waiting ? t.txt : t.mut }}>
        {waiting ? tr('said.guess.whoWrote', { n: waiting }) : isComplete(state) ? tr('said.guess.allSet') : tr('said.guess.tapAnswer')}
      </Text>
      {crowd ? (
        <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={{ gap: 8, paddingHorizontal: 2, paddingVertical: 2 }}>
          {chips}
        </ScrollView>
      ) : (
        <View style={{ flexDirection: 'row', flexWrap: 'wrap', justifyContent: 'center', gap: 8 }}>{chips}</View>
      )}
    </View>
  );
}

const SuspectChip = memo(function SuspectChip({
  member,
  slot,
  onPick,
}: {
  member: Member;
  /** The answer they're on (its number), 0: none. */
  slot: number;
  onPick: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const used = slot > 0;
  const bump = usePop(slot, 1.08, used);
  return (
    <Animated.View style={bump}>
      <PressableScale
        scale={0.94}
        accessibilityRole="button"
        accessibilityLabel={used ? tr('said.guess.placed', { name: member.name, n: slot }) : tr('said.guess.assign', { name: member.name })}
        onPress={() => onPick(member.id)}
        style={[
          {
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            height: 48,
            paddingLeft: 6,
            paddingRight: used ? 8 : 16,
            borderRadius: t.radius.pill,
            backgroundColor: used ? t.card2 : t.card,
          },
          !used && t.shadow,
        ]}>
        <View style={{ opacity: used ? 0.55 : 1 }}>
          <Avatar member={member} size={34} />
        </View>
        <Text numberOfLines={1} style={{ maxWidth: 110, fontFamily: t.font.semibold, fontSize: 15, color: used ? t.mut : t.txt }}>
          {firstName(member.name)}
        </Text>
        {used && <NumberDisc n={slot} />}
      </PressableScale>
    </Animated.View>
  );
});

/** Zeste, the instigator, eggs you on from the side: a line for the start, along the way, and once all is placed. */
export function ZesteSays({ seed, done, total }: { seed: number; done: number; total: number }) {
  const t = useTheme();
  const tr = useT();
  const stage = done === 0 ? 's' : done >= total ? 'd' : 'm';
  const lines =
    stage === 's' ? (['s1', 's2', 's3', 's4'] as const) : stage === 'd' ? (['d1', 'd2', 'd3'] as const) : (['m1', 'm2', 'm3', 'm4'] as const);
  const line = pickLine(lines, seed * 31 + done);
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
      <Mascot
        who="zeste"
        mood={stage === 's' ? 'think' : stage === 'd' ? 'happy' : 'accuse'}
        size={64}
        style={{ marginVertical: -6, marginLeft: -6 }}
      />
      {/* The bubble's tail, then the bubble. */}
      <View
        style={{
          width: 0,
          height: 0,
          borderTopWidth: 7,
          borderBottomWidth: 7,
          borderRightWidth: 9,
          borderTopColor: 'transparent',
          borderBottomColor: 'transparent',
          borderRightColor: t.card,
          marginRight: -4,
        }}
      />
      <View style={[{ flex: 1, paddingVertical: 10, paddingHorizontal: 14, borderRadius: 16, backgroundColor: t.card }, t.shadow]}>
        <Animated.Text key={line} entering={motion.enter} style={{ fontFamily: t.font.semibold, fontSize: 14, lineHeight: 19, color: t.txt }}>
          {tr(`said.zeste.${line}`)}
        </Animated.Text>
      </View>
    </View>
  );
}

/** My guesses once sent: each answer and who I said wrote it. */
export function GuessRecap({ answers, guesses, byId }: { answers: SaidAnswer[]; guesses: SaidGuess[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const said = new Map(guesses.map((g) => [g.answer_id, g.member_id]));
  const rows = answers.filter((a) => said.has(a.id));
  if (!rows.length) return null;
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <Label>{tr('said.recap.title')}</Label>
      {rows.map((a, i) => {
        const m = byId.get(said.get(a.id)!);
        return (
          <View
            key={a.id}
            accessible
            accessibilityLabel={`${a.text ?? tr('said.guess.hidden')}. ${tr('said.guess.on', { name: m?.name ?? '?' })}`}
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
            <NumberDisc n={i + 1} />
            <View style={{ flex: 1 }}>
              {a.text != null ? (
                <Text numberOfLines={2} style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}>
                  {a.text}
                </Text>
              ) : (
                <AnswerText answer={a} size={15} />
              )}
            </View>
            <ArrowRight size={14} color={t.mut} />
            <View style={{ alignItems: 'center', width: 56, gap: 2 }}>
              <Avatar member={m} size={30} />
              <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 11, color: t.mut }}>
                {m ? firstName(m.name) : '?'}
              </Text>
            </View>
          </View>
        );
      })}
    </Card>
  );
}

// ---------------------------------------------------------------------------------------------------------------------
// The room's open question

/**
 * The open said question of a room: write (and rewrite), then guess, then wait for the others.
 * `top`: the header and the question, drawn by the Vote tab. `waitList(done, title)`: who we're waiting for (nudges, skips).
 */
export function SaidRound({
  round,
  said,
  me,
  byId,
  top,
  waitList,
  onVoted,
}: {
  round: Round;
  said: SaidView;
  me: number | undefined;
  byId: Map<number, Member>;
  top: ReactNode;
  waitList: (done: number[], title: string) => ReactNode;
  onVoted: () => void;
}) {
  if (said.phase === 'writing') return <SaidWriting round={round} said={said} byId={byId} top={top} waitList={waitList} />;
  if (round.my_vote == null && said.phase === 'guessing')
    return <SaidGuessing round={round} said={said} me={me} byId={byId} top={top} onVoted={onVoted} />;
  return <SaidGuessed round={round} said={said} byId={byId} top={top} waitList={waitList} />;
}

function SaidWriting({
  round,
  said,
  byId,
  top,
  waitList,
}: {
  round: Round;
  said: SaidView;
  byId: Map<number, Member>;
  top: ReactNode;
  waitList: (done: number[], title: string) => ReactNode;
}) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const mine = said.my_answer;
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(mine?.text ?? '');
  const [error, setError] = useState<string | null>(null);
  const send = useMutation({
    meta: { quiet: true },
    mutationFn: (answer: string) => answerCurrent(answer),
    onSuccess: (r) => {
      setEditing(false);
      setError(null);
      if (r && typeof r === 'object' && 'id' in r) qc.setQueryData(roundKeys.current, r);
    },
    onError: (e) => {
      const late = e instanceof ApiError && (e.code === 'not_writing' || e.code === 'not_said' || e.code === 'no_open_round');
      setError(tr(late ? 'said.write.tooLate' : 'said.write.failed'));
    },
    onSettled: () => qc.invalidateQueries({ queryKey: roundKeys.current }),
  });
  const ok = text.trim().length > 0 && !send.isPending && text.trim() !== mine?.text;
  const submit = () => ok && send.mutate(text.trim());
  const writing = !mine || editing;
  const done = said.writer_ids.length;
  return (
    <Screen
      footer={
        writing && (
          <AnswerButtons
            editing={editing}
            ok={ok}
            busy={send.isPending}
            onSubmit={submit}
            onCancel={() => {
              setEditing(false);
              setError(null);
              setText(mine?.text ?? '');
            }}
          />
        )
      }>
      {top}
      {writing ? (
        <>
          {done > 0 && (
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
              <AvatarStack members={said.writer_ids.map((id) => byId.get(id))} max={6} />
              <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
                {tr('said.write.written', { count: done, done, total: round.member_count })}
              </Text>
            </View>
          )}
          <AnswerField value={text} onChange={setText} onSubmit={submit} autoFocus={editing} error={error} />
        </>
      ) : (
        <>
          <Animated.View entering={motion.rise}>
            <MyAnswer
              text={mine.text}
              onEdit={() => {
                setText(mine.text);
                setEditing(true);
              }}
            />
          </Animated.View>
          <Text style={{ marginTop: -4, paddingHorizontal: 4, fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
            {tr('said.write.next')}
          </Text>
          {waitList(said.writer_ids, tr('said.write.written', { count: done, done, total: round.member_count }))}
        </>
      )}
    </Screen>
  );
}

function SaidGuessing({
  round,
  said,
  me,
  byId,
  top,
  onVoted,
}: {
  round: Round;
  said: SaidView;
  me: number | undefined;
  byId: Map<number, Member>;
  top: ReactNode;
  onVoted: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const answers = useMemo(() => toGuess(said), [said]);
  // The writers but me (members are stable objects, so the chips only redraw when their own state changes).
  const people = suspectIds(said, me).flatMap((id) => {
    const m = byId.get(id);
    return m ? [m] : [];
  });
  const [state, dispatch] = useMatchGame(answers.map((a) => a.id));
  const complete = isComplete(state);
  const nothing = answers.length === 0;
  const vote = useMutation({
    meta: { quiet: true },
    mutationFn: (guesses: SaidGuess[]) => voteCurrent({ guesses }),
    onSuccess: onVoted,
    // The writers changed meanwhile (someone left): the refreshed room shows who to place now.
    onError: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
    onSettled: () => qc.invalidateQueries({ queryKey: roundKeys.current }),
  });
  const send = () => (complete || nothing) && vote.mutate(toGuesses(state));
  const bump = usePop(complete, 1.04, complete);
  const [toReport, setToReport] = useState<ModTarget | null>(null);
  const onReport = useCallback((id: number) => setToReport({ kind: 'answer', id }), []);
  const done = placed(state);
  return (
    <Screen
      footer={
        <>
          {!nothing && <SuspectTray people={people} state={state} dispatch={dispatch} />}
          {vote.isError && (
            <Text style={{ textAlign: 'center', fontFamily: t.font.semibold, fontSize: 13, color: t.tagInk }}>{tr('said.guess.failed')}</Text>
          )}
          <Animated.View style={bump}>
            <Button disabled={!complete && !nothing} loading={vote.isPending} onPress={send}>
              {complete || nothing ? tr('said.guess.submit') : tr('said.guess.progress', { done, total: answers.length })}
            </Button>
          </Animated.View>
        </>
      }>
      {top}
      {round.voter_ids.length > 0 && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} max={6} />
          <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
            {tr('said.recap.voted', { count: round.voter_ids.length, done: round.voter_ids.length, total: round.member_count })}
          </Text>
        </View>
      )}
      {nothing ? (
        <Card style={{ padding: 16 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 21, color: t.mut }}>{tr('said.guess.nothing')}</Text>
        </Card>
      ) : (
        <>
          <ZesteSays seed={round.id} done={done} total={answers.length} />
          <Tip id="said" text={tr('said.guess.tip')} />
          <MatchBoard answers={answers} state={state} byId={byId} dispatch={dispatch} onReport={onReport} />
          {done > 0 && (
            <Pressable
              accessibilityRole="button"
              onPress={() => dispatch({ type: 'reset' })}
              hitSlop={8}
              style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 6, minHeight: 44, paddingHorizontal: 12 }}>
              <RotateCcw size={14} color={t.mut} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('said.guess.restart')}</Text>
            </Pressable>
          )}
        </>
      )}
      <ModerationDialog target={toReport} onClose={() => setToReport(null)} />
    </Screen>
  );
}

function SaidGuessed({
  round,
  said,
  byId,
  top,
  waitList,
}: {
  round: Round;
  said: SaidView;
  byId: Map<number, Member>;
  top: ReactNode;
  waitList: (done: number[], title: string) => ReactNode;
}) {
  const tr = useT();
  const answers = toGuess(said);
  const done = round.voter_ids.length;
  return (
    <Screen>
      {top}
      {said.my_answer && <MyAnswer text={said.my_answer.text} />}
      <GuessRecap answers={answers} guesses={said.my_guesses ?? []} byId={byId} />
      {waitList(round.voter_ids, tr('said.recap.voted', { count: done, done, total: round.member_count }))}
    </Screen>
  );
}

// ---------------------------------------------------------------------------------------------------------------------
// The reveal

/** Rounds whose reveal I've watched (on this device): coming back shows the whole result at once. */
const SEEN_KEY = 'qdn.saidSeen';
let seenIds: number[] | null = null;
const seenListeners = new Set<() => void>();
storage
  .get(SEEN_KEY)
  .then((saved) => {
    seenIds = saved ? (JSON.parse(saved) as number[]) : [];
  })
  .catch(() => {
    seenIds = [];
  })
  .finally(() => seenListeners.forEach((l) => l()));

function markSeen(roundId: number) {
  if (seenIds?.includes(roundId)) return;
  seenIds = [...(seenIds ?? []), roundId].slice(-60);
  storage.set(SEEN_KEY, JSON.stringify(seenIds)).catch(() => {});
  seenListeners.forEach((l) => l());
}

/** Seen, not seen, or null while the list loads (nothing drawn yet: no reveal flashing before the full result). */
function useSeen(roundId: number): boolean | null {
  return useSyncExternalStore(
    (l) => {
      seenListeners.add(l);
      return () => seenListeners.delete(l);
    },
    () => (seenIds ? seenIds.includes(roundId) : null),
  );
}

// One step of the reveal: the guesses come in one by one, then the author's card flips.
const GUESS_IN = 250;
const GUESS_STEP = 120;
const FLIP_MS = 460;
const flipAt = (groups: number) => GUESS_IN + Math.max(1, groups) * GUESS_STEP + 650;

/**
 * The authors, one answer at a time (the server's order: most found first, best hidden last). `onDone`: past the last
 * one, or "Tout voir".
 */
export function SaidReveal({ items, byId, onDone }: { items: RevealItem[]; byId: Map<number, Member>; onDone: () => void }) {
  const t = useTheme();
  const tr = useT();
  const [step, setStep] = useState(0);
  const item = items[Math.min(step, items.length - 1)];
  if (!item) return null;
  const last = step >= items.length - 1;
  return (
    <Card style={{ padding: 18, gap: 16 }}>
      <View
        style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}
        accessibilityLabel={tr('said.reveal.step', { n: step + 1, total: items.length })}>
        {items.map((it, i) => (
          <View key={it.id} style={{ flex: 1, maxWidth: 40, height: 4, borderRadius: 2, backgroundColor: i <= step ? t.accInk : t.card2 }} />
        ))}
        <View style={{ flex: 1 }} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, fontVariant: ['tabular-nums'] }}>
          {tr('said.reveal.step', { n: step + 1, total: items.length })}
        </Text>
      </View>
      <RevealStep key={item.id} item={item} byId={byId} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        {!last && (
          <Pressable
            accessibilityRole="button"
            onPress={onDone}
            hitSlop={8}
            style={{ minHeight: 44, justifyContent: 'center', paddingHorizontal: 6 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('said.reveal.all')}</Text>
          </Pressable>
        )}
        <View style={{ flex: 1 }} />
        <PressableScale
          accessibilityRole="button"
          onPress={() => (last ? onDone() : setStep(step + 1))}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            height: 46,
            paddingHorizontal: 20,
            borderRadius: t.radius.pill,
            backgroundColor: last ? t.acc : t.card2,
          }}>
          {last && <Trophy size={16} color={t.accTxt} />}
          <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: last ? t.accTxt : t.txt }}>
            {tr(last ? 'said.reveal.scores' : 'said.reveal.next')}
          </Text>
          {!last && <ArrowRight size={16} color={t.txt} />}
        </PressableScale>
      </View>
    </Card>
  );
}

function RevealStep({ item, byId }: { item: RevealItem; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const groups = guessGroups(item);
  const at = flipAt(groups.length);
  const author = byId.get(item.author_id);
  // A small knock when the author shows (not on the web, nor under reduced motion).
  const reduced = useReducedMotion();
  useEffect(() => {
    if (reduced) return;
    const id = setTimeout(thud, at + FLIP_MS / 2);
    return () => clearTimeout(id);
  }, [at, reduced]);
  return (
    <View style={{ gap: 16 }}>
      <View style={{ flexDirection: 'row', gap: 10 }}>
        <Quote size={22} color={t.accInk} fill={t.accInk} style={{ marginTop: 3 }} />
        <View style={{ flex: 1 }}>
          {item.text != null ? (
            <Text style={{ fontFamily: t.font.display, fontSize: 24, lineHeight: 29, letterSpacing: -0.5, color: t.txt }}>{item.text}</Text>
          ) : (
            <AnswerText answer={item} size={18} />
          )}
        </View>
      </View>
      <View style={{ gap: 8 }}>
        <Label>{tr('said.reveal.named')}</Label>
        {groups.length ? (
          groups.map((g, i) => <GuessRow key={g.member_id} group={g} byId={byId} delay={GUESS_IN + i * GUESS_STEP} settle={at + FLIP_MS} />)
        ) : (
          <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>{tr('said.reveal.noGuess')}</Text>
        )}
      </View>
      <View style={{ height: 1, backgroundColor: t.line }} />
      <View
        accessible
        accessibilityLabel={`${tr('said.reveal.it')} ${author?.name ?? '?'}. ${tr('said.reveal.fooled', { count: item.fooled })}. ${tr('said.reveal.found', { count: item.found })}`}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
        <Flip member={author} at={at} />
        <View style={{ flex: 1, gap: 4 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('said.reveal.it')}</Text>
          <Animated.Text
            entering={FadeIn.delay(at + FLIP_MS / 2)
              .duration(160)
              .reduceMotion(sys)}
            numberOfLines={1}
            style={{ fontFamily: t.font.display, fontSize: 24, letterSpacing: -0.5, color: t.txt }}>
            {author?.name ?? '?'}
          </Animated.Text>
        </View>
        <Animated.View
          entering={FadeInDown.delay(at + FLIP_MS)
            .duration(260)
            .easing(LAYOUT_EASE)
            .reduceMotion(sys)}>
          <PointsPill points={item.fooled} />
        </Animated.View>
      </View>
      <Animated.View
        entering={FadeIn.delay(at + FLIP_MS + 180)
          .duration(240)
          .reduceMotion(sys)}
        style={{ gap: 6 }}>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 19, color: t.mut }}>
          {tr('said.reveal.fooled', { count: item.fooled })} · {tr('said.reveal.found', { count: item.found })}
        </Text>
        <MyVerdict item={item} byId={byId} />
      </Animated.View>
    </View>
  );
}

/** One name the others put on the answer, and who did; once the author is out, the right one lights up, the rest fade. */
function GuessRow({
  group: g,
  byId,
  delay,
  settle,
  k = 1,
}: {
  group: ReturnType<typeof guessGroups>[number];
  byId: Map<number, Member>;
  delay: number;
  settle: number;
  /** A size factor (the big screen). */
  k?: number;
}) {
  const t = useTheme();
  const tr = useT();
  const m = byId.get(g.member_id);
  const reduced = useReducedMotion();
  const fade = useSharedValue(1);
  useEffect(() => {
    if (!g.right) fade.value = reduced ? 0.45 : withDelay(settle, withTiming(0.45, { duration: 260, easing: EASE }));
  }, [g.right, settle, reduced, fade]);
  const dim = useAnimatedStyle(() => ({ opacity: fade.value }));
  return (
    <Animated.View entering={FadeInDown.delay(delay).duration(240).easing(LAYOUT_EASE).reduceMotion(sys)}>
      <Animated.View
        style={[
          {
            flexDirection: 'row',
            alignItems: 'center',
            gap: 10 * k,
            minHeight: 48 * k,
            paddingHorizontal: 10 * k,
            borderRadius: 14 * k,
            backgroundColor: t.card2,
          },
          dim,
        ]}>
        <Avatar member={m} size={Math.round(30 * k)} />
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: Math.round(15 * k), color: t.txt }}>
          {m?.name ?? '?'}
        </Text>
        <AvatarStack members={g.voters.map((id) => byId.get(id))} size={Math.round(22 * k)} ring={t.card2} max={4} />
        <Text
          accessibilityLabel={tr('said.result.found', { count: g.voters.length })}
          style={{ fontFamily: t.font.display, fontSize: Math.round(15 * k), color: t.mut }}>
          ×{g.voters.length}
        </Text>
        {g.right && (
          <Animated.View
            entering={FadeIn.delay(settle).duration(200).reduceMotion(sys)}
            pointerEvents="none"
            style={{
              position: 'absolute',
              left: -2,
              top: -2,
              right: -2,
              bottom: -2,
              borderRadius: 14 * k + 2,
              borderWidth: Math.round(2 * k),
              borderColor: t.accInk,
            }}
          />
        )}
      </Animated.View>
    </Animated.View>
  );
}

/** The author's face, face down ("?") until it flips over. */
function Flip({ member, at, size: S = 64 }: { member: Member | undefined; at: number; size?: number }) {
  const t = useTheme();
  const reduced = useReducedMotion();
  const p = useSharedValue(reduced ? 1 : 0);
  useEffect(() => {
    if (reduced) return;
    p.value = withDelay(at, withTiming(1, { duration: FLIP_MS, easing: EASE }));
  }, [at, reduced, p]);
  const pop = useSharedValue(1);
  useEffect(() => {
    if (reduced) return;
    pop.value = withDelay(at + FLIP_MS, withSequence(withTiming(1.12, { duration: 110, easing: EASE }), withSpring(1, BOUNCE)));
  }, [at, reduced, pop]);
  const front = useAnimatedStyle(() => ({
    opacity: p.value < 0.5 ? 1 : 0,
    transform: [{ perspective: 600 }, { rotateY: `${p.value * 180}deg` }],
  }));
  const back = useAnimatedStyle(() => ({
    opacity: p.value < 0.5 ? 0 : 1,
    transform: [{ perspective: 600 }, { rotateY: `${p.value * 180 - 180}deg` }, { scale: pop.value }],
  }));
  return (
    <View style={{ width: S, height: S }}>
      <Animated.View
        style={[
          { position: 'absolute', width: S, height: S, borderRadius: S / 2, alignItems: 'center', justifyContent: 'center', backgroundColor: t.acc },
          front,
        ]}>
        <Text style={{ fontFamily: t.font.display, fontSize: Math.round(S * 0.47), color: t.accTxt }}>?</Text>
      </Animated.View>
      <Animated.View style={[{ position: 'absolute' }, back]}>
        <Avatar member={member} size={S} />
      </Animated.View>
    </View>
  );
}

/** "+3": what the author scored on this answer (the people it fooled). */
function PointsPill({ points, k = 1 }: { points: number; k?: number }) {
  const t = useTheme();
  const on = points > 0;
  return (
    <View
      style={{
        minWidth: 52 * k,
        height: 40 * k,
        paddingHorizontal: 12 * k,
        borderRadius: 20 * k,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: on ? t.acc : t.card2,
      }}>
      <Text style={{ fontFamily: t.font.display, fontSize: Math.round(20 * k), color: on ? t.accTxt : t.mut }}>+{points}</Text>
    </View>
  );
}

/** How I did on this one: found it, missed it (and who I'd said), or it was mine. */
function MyVerdict({ item, byId }: { item: RevealItem; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  if (!item.own && item.myGuess == null) return null;
  const right = item.myGuess === item.author_id;
  const [Icon, color, text] = item.own
    ? [Quote, t.accInk, tr('said.reveal.own')]
    : right
      ? [Check, t.accInk, tr('said.reveal.right')]
      : [X, t.tagInk, tr('said.reveal.wrong', { name: byId.get(item.myGuess!)?.name ?? '?' })];
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
      <Icon size={16} color={color} strokeWidth={3} />
      <Text style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 15, color }}>{text}</Text>
    </View>
  );
}

// ---------------------------------------------------------------------------------------------------------------------
// The big screen's reveal (a party's TV): nobody taps a TV, so the screen moves on by itself (`screenRevealSteps`).

/** Once the author is out: time to read, point and laugh before the next answer. */
const SCREEN_HOLD = 4500;
/** How long the big screen stays on each answer (ms), in reveal order. */
export const screenRevealSteps = (items: RevealItem[]) => items.map((it) => flipAt(guessGroups(it).length) + FLIP_MS + SCREEN_HOLD);
/** The big screen shows this many names under an answer (the right one always among them), then "and 2 more". */
const SCREEN_GROUPS = 3;

/** Answer `step` of the reveal, big: the progress, the answer, who was named, then the author flips. `k`: a size factor. */
export function SaidScreenReveal({
  items,
  step,
  byId,
  k,
  wide,
}: {
  items: RevealItem[];
  step: number;
  byId: Map<number, Member>;
  k: number;
  /** The answer on the left, the names and the author on the right. */
  wide: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  const item = items[Math.min(step, items.length - 1)];
  if (!item) return null;
  return (
    <View style={{ gap: 28 * k }}>
      <View
        accessible
        accessibilityLabel={tr('said.reveal.step', { n: step + 1, total: items.length })}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 8 * k }}>
        {items.map((it, i) => (
          <View key={it.id} style={{ flex: 1, maxWidth: 72 * k, height: 6 * k, borderRadius: 3 * k, backgroundColor: i <= step ? t.accInk : t.card2 }} />
        ))}
        <View style={{ flex: 1 }} />
        <Text style={{ fontFamily: t.font.display, fontSize: Math.round(22 * k), color: t.mut, fontVariant: ['tabular-nums'] }}>
          {tr('said.reveal.step', { n: step + 1, total: items.length })}
        </Text>
      </View>
      <ScreenRevealStep key={item.id} item={item} byId={byId} k={k} wide={wide} />
    </View>
  );
}

function ScreenRevealStep({ item, byId, k, wide }: { item: RevealItem; byId: Map<number, Member>; k: number; wide: boolean }) {
  const t = useTheme();
  const tr = useT();
  const all = guessGroups(item);
  const at = flipAt(all.length);
  // The most named first; the right one stays in even when it comes after the cut.
  let groups = all.slice(0, SCREEN_GROUPS);
  const right = all.find((g) => g.right);
  if (right && !groups.includes(right)) groups = [...groups.slice(0, SCREEN_GROUPS - 1), right];
  const rest = all.length - groups.length;
  const author = byId.get(item.author_id);
  const f = (n: number) => Math.round(n * k);
  const long = (item.text?.length ?? 0) > 80;
  return (
    <View style={{ flexDirection: wide ? 'row' : 'column', alignItems: wide ? 'center' : 'stretch', gap: 48 * k }}>
      <Animated.View entering={motion.question} style={{ flex: wide ? 1.15 : undefined, flexDirection: 'row', gap: 16 * k }}>
        <Quote size={f(40)} color={t.accInk} fill={t.accInk} style={{ marginTop: 10 * k }} />
        <View style={{ flex: 1 }}>
          {item.text != null ? (
            <Text
              style={{
                fontFamily: t.font.display,
                fontSize: f(long ? 46 : 60),
                lineHeight: f(long ? 52 : 66),
                letterSpacing: -1,
                color: t.txt,
              }}>
              {item.text}
            </Text>
          ) : (
            <AnswerText answer={item} size={f(32)} />
          )}
        </View>
      </Animated.View>
      <View style={{ flex: wide ? 1 : undefined, gap: 12 * k }}>
        <Label k={1.9 * k}>{tr('said.reveal.named')}</Label>
        {groups.length ? (
          groups.map((g, i) => (
            <GuessRow key={g.member_id} group={g} byId={byId} delay={GUESS_IN + i * GUESS_STEP} settle={at + FLIP_MS} k={1.75 * k} />
          ))
        ) : (
          <Text style={{ fontFamily: t.font.regular, fontSize: f(26), color: t.mut }}>{tr('said.reveal.noGuess')}</Text>
        )}
        {rest > 0 && <Text style={{ fontFamily: t.font.semibold, fontSize: f(22), color: t.mut }}>{tr('said.party.more', { count: rest })}</Text>}
        <View style={{ height: 1, marginVertical: 6 * k, backgroundColor: t.line }} />
        <View
          accessible
          accessibilityLabel={`${tr('said.reveal.it')} ${author?.name ?? '?'}. ${tr('said.reveal.fooled', { count: item.fooled })}. ${tr('said.reveal.found', { count: item.found })}`}
          style={{ flexDirection: 'row', alignItems: 'center', gap: 20 * k }}>
          <Flip member={author} at={at} size={f(120)} />
          <View style={{ flex: 1, gap: 4 * k }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: f(26), color: t.mut }}>{tr('said.reveal.it')}</Text>
            <Animated.Text
              entering={FadeIn.delay(at + FLIP_MS / 2)
                .duration(160)
                .reduceMotion(sys)}
              numberOfLines={1}
              style={{ fontFamily: t.font.display, fontSize: f(58), lineHeight: f(64), letterSpacing: -1.5, color: t.txt }}>
              {author?.name ?? '?'}
            </Animated.Text>
          </View>
          <Animated.View
            entering={FadeInDown.delay(at + FLIP_MS)
              .duration(260)
              .easing(LAYOUT_EASE)
              .reduceMotion(sys)}>
            <PointsPill points={item.fooled} k={1.9 * k} />
          </Animated.View>
        </View>
        <Animated.Text
          entering={FadeIn.delay(at + FLIP_MS + 180)
            .duration(240)
            .reduceMotion(sys)}
          style={{ fontFamily: t.font.semibold, fontSize: f(24), lineHeight: f(32), color: t.mut }}>
          {tr('said.reveal.fooled', { count: item.fooled })} · {tr('said.reveal.found', { count: item.found })}
        </Animated.Text>
      </View>
    </View>
  );
}

/** Every answer with its author at once (after the reveal, or with reduced motion). */
export function SaidAnswerList({ items, byId }: { items: RevealItem[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>{tr('said.result.list')}</Text>
      <Card style={{ padding: 12, gap: 8 }}>
        {items.map((it) => {
          const m = byId.get(it.author_id);
          const verdict = it.own || it.myGuess == null ? null : it.myGuess === it.author_id;
          return (
            <View
              key={it.id}
              accessible
              accessibilityLabel={`${tr('said.reveal.author', { name: m?.name ?? '?', text: it.text ?? tr('said.guess.hidden') })}. ${tr('said.reveal.fooled', { count: it.fooled })}. ${tr('said.reveal.found', { count: it.found })}${verdict == null ? '' : `. ${verdict ? tr('said.reveal.right') : tr('said.reveal.wrong', { name: byId.get(it.myGuess!)?.name ?? '?' })}`}`}
              style={{ padding: 12, gap: 10, borderRadius: 14, backgroundColor: t.card2 }}>
              <AnswerText answer={it} size={15} />
              <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                <Avatar member={m} size={24} />
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 14, color: t.txt }}>
                  {m?.name ?? '?'}
                  {it.own && <Text style={{ fontFamily: t.font.regular, color: t.mut }}>{` · ${tr('said.result.you')}`}</Text>}
                </Text>
                <View style={{ flex: 1 }} />
                <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
                  {tr('said.result.fooled', { count: it.fooled })} · {tr('said.result.found', { count: it.found })}
                </Text>
                {verdict != null &&
                  (verdict ? <Check size={16} color={t.accInk} strokeWidth={3} /> : <X size={16} color={t.tagInk} strokeWidth={3} />)}
              </View>
            </View>
          );
        })}
      </Card>
    </View>
  );
}

/**
 * Points, best first; the same points share a place; my line stands out. The big screen: `k` (a size factor) and
 * `columns` (2: the list split in two, top to bottom then the second column).
 */
export function SaidScoreboard({
  scores,
  byId,
  me,
  k = 1,
  columns = 1,
}: {
  scores: SaidResult['scores'];
  byId: Map<number, Member>;
  me: number | undefined;
  k?: number;
  columns?: 1 | 2;
}) {
  const t = useTheme();
  const tr = useT();
  const list = scores.filter((s) => byId.has(s.member_id));
  const at = places(list);
  const nobody = !list.some((s) => s.points > 0);
  const row = (s: SaidResult['scores'][number], i: number) => (
    <ScoreRow key={s.member_id} score={s} member={byId.get(s.member_id)} place={at[i]} i={i} mine={s.member_id === me} k={k} />
  );
  const half = Math.ceil(list.length / 2);
  return (
    <View style={{ gap: 10 * k }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: Math.round(17 * k), color: t.txt }}>{tr('said.result.board')}</Text>
      <Card style={{ padding: 10 * k, gap: 4 * k }}>
        {nobody && (
          <Text style={{ padding: 6 * k, fontFamily: t.font.regular, fontSize: Math.round(14 * k), color: t.mut }}>{tr('said.result.nobody')}</Text>
        )}
        {columns === 2 && list.length > 1 ? (
          <View style={{ flexDirection: 'row', gap: 12 * k }}>
            <View style={{ flex: 1, gap: 4 * k }}>{list.slice(0, half).map(row)}</View>
            <View style={{ flex: 1, gap: 4 * k }}>{list.slice(half).map((s, i) => row(s, half + i))}</View>
          </View>
        ) : (
          list.map(row)
        )}
      </Card>
    </View>
  );
}

const ScoreRow = memo(function ScoreRow({
  score: s,
  member: m,
  place,
  i,
  mine,
  k,
}: {
  score: SaidResult['scores'][number];
  member: Member | undefined;
  place: number;
  i: number;
  mine: boolean;
  k: number;
}) {
  const t = useTheme();
  const tr = useT();
  const top = place === 1 && s.points > 0;
  const f = (n: number) => Math.round(n * k);
  return (
    <Animated.View
      entering={motion.stagger(i)}
      accessible
      accessibilityLabel={tr('said.result.row', {
        place,
        name: mine ? `${m?.name ?? '?'} (${tr('said.result.you')})` : (m?.name ?? '?'),
        points: s.points,
        found: tr('said.result.found', { count: s.found }),
        fooled: tr('said.result.fooled', { count: s.fooled }),
      })}
      style={{
        flexDirection: 'row',
        alignItems: 'center',
        gap: 10 * k,
        minHeight: 56 * k,
        paddingHorizontal: 10 * k,
        borderRadius: 16 * k,
        backgroundColor: mine ? t.accSoft : 'transparent',
      }}>
      <Text style={{ width: 22 * k, textAlign: 'center', fontFamily: t.font.display, fontSize: f(16), color: top ? t.accInk : t.mut }}>{place}</Text>
      <Avatar member={m} size={f(36)} />
      <View style={{ flex: 1, minWidth: 0, gap: 1 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 * k }}>
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: f(15), color: t.txt }}>
            {m?.name ?? '?'}
          </Text>
          {top && <Trophy size={f(14)} color={m?.color ?? t.accInk} />}
          {mine && <Text style={{ fontFamily: t.font.semibold, fontSize: f(12), color: t.accInk }}>{tr('said.result.you')}</Text>}
        </View>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: f(12), color: t.mut }}>
          {tr('said.result.found', { count: s.found })} · {tr('said.result.fooled', { count: s.fooled })}
        </Text>
      </View>
      <Text style={{ fontFamily: t.font.display, fontSize: f(22), color: s.points ? t.txt : t.mut }}>
        {s.points}
        <Text style={{ fontFamily: t.font.semibold, fontSize: f(12), color: t.mut }}> {tr('results.pts')}</Text>
      </Text>
    </Animated.View>
  );
});

/**
 * A finished said question: the reveal the first time (on this device), then the scoreboard and every answer with its
 * author. Reduced motion: straight to the full result.
 */
export function SaidResultView({ roundId, said, byId, me }: { roundId: number; said: SaidView; byId: Map<number, Member>; me: number | undefined }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const seen = useSeen(roundId);
  const [replay, setReplay] = useState(false);
  const items = revealItems(said, me);
  if (!said.result || seen == null) return null;
  if ((!seen || replay) && !reduced && items.length > 0)
    return (
      <SaidReveal
        items={items}
        byId={byId}
        onDone={() => {
          markSeen(roundId);
          setReplay(false);
        }}
      />
    );
  return (
    <Animated.View entering={motion.enter} style={{ gap: 20 }}>
      <SaidScoreboard scores={said.result.scores} byId={byId} me={me} />
      {items.length > 0 && <SaidAnswerList items={items} byId={byId} />}
      {!reduced && items.length > 0 && (
        <Pressable
          accessibilityRole="button"
          onPress={() => setReplay(true)}
          hitSlop={8}
          style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 6, minHeight: 44, paddingHorizontal: 12 }}>
          <RotateCcw size={14} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('said.reveal.replay')}</Text>
        </Pressable>
      )}
    </Animated.View>
  );
}
