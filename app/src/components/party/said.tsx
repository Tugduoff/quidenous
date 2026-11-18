// « Qui a dit ça ? » at a party (docs/SAID.md, Party): the phone (write, guess, the result) and the big screen (the
// prompt while people write, the answers while they guess, the authors one by one, the scoreboard). Built from the
// room's pieces (components/said.tsx); the party's own calls are in lib/party.ts.
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Check, Hourglass, RotateCcw } from 'lucide-react-native';
import { memo, type ReactNode, useCallback, useMemo, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { ScreenCounts } from '@/components/result/live-reactions';
import { Countdown, Reactions, VotedLine } from '@/components/party/party';
import { AnswerButtons } from '@/components/said/answer-buttons';
import { AnswerField } from '@/components/said/answer-field';
import { AnswerText } from '@/components/said/answer-text';
import { GuessRecap } from '@/components/said/guess-recap';
import { MatchBoard } from '@/components/said/match-board';
import { MyAnswer } from '@/components/said/my-answer';
import { NumberDisc } from '@/components/said/number-disc';
import { SaidResultView } from '@/components/said/said';
import { SaidScoreboard } from '@/components/said/said-scoreboard';
import { SaidScreenReveal } from '@/components/said/said-screen-reveal';
import { SuspectTray } from '@/components/said/suspect-tray';
import { ZesteSays } from '@/components/said/zeste-says';
import { useMatchGame } from '@/lib/hooks/use-match-game';
import { Screen } from '@/components/ui/screen';
import { Tip } from '@/components/ui/tip';
import { useT } from '@/lib/i18n';
import { motion, usePop } from '@/lib/motion';
import { type Party, partyApi, partyError, partyKeys, type PartyRound, useSetParty } from '@/lib/api/party';
import { isComplete, placed, revealItems, screenRevealSteps, suspects as suspectIds, toGuess, toGuesses } from '@/lib/utils/said';
import { tidy } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Tag } from '@/components/ui/tag';
import { Card } from '@/components/ui/card';
import { Display, ErrorText } from '@/components/ui/text';
import type { Member } from '@/lib/api/group';
import type { SaidAnswer, SaidGuess, SaidView } from '@/lib/api/rounds';
import { useFeatures } from '@/lib/api/features';
import { useCountdown } from '@/lib/hooks/use-countdown';
import { useScreenStep } from '@/lib/hooks/use-party';
import { useRefetchAtZero } from '@/lib/hooks/use-refetch-at-zero';
import { phaseSeconds } from '@/lib/utils/party';

/** The players by id. Kept while the players don't change (refetches share unchanged parts), so lists don't redraw. */
function usePlayers(party: Party) {
  const players = party.players;
  return useMemo(() => new Map<number, Member>(players.map((m) => [m.id, m])), [players]);
}

/** The answers to guess, in order: the board starts over only if that changes (an answer taken away by someone leaving). */
export const saidBoardKey = (said: SaidView) =>
  toGuess(said)
    .map((a) => a.id)
    .join(',');

// ---------------------------------------------------------------------------------------------------------------------
// The phone

/**
 * The open said question on a player's phone: write (and rewrite until the writing deadline), then match the answers to
 * their authors, then wait for the others. `top`: the header and the question. Keyed by the parent on round, phase and
 * board, so each phase starts fresh.
 */
export function PartySaidVoting({ party, round, said, top }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode }) {
  const me = party.me?.member_id;
  // Arrived after the question opened: plays from the next one.
  const late = me == null || !round.eligible.includes(me);
  if (said.phase === 'writing') return <Writing party={party} round={round} said={said} top={top} late={late} />;
  const guessed = round.my_vote != null || (me != null && round.voted.includes(me));
  if (!late && !guessed) return <Guessing party={party} round={round} said={said} top={top} />;
  return <Guessed party={party} round={round} said={said} top={top} late={late} />;
}

/** The countdown of the current phase, refetching once it runs out in case the server's event got lost. */
function usePhaseClock(party: Party, round: PartyRound) {
  const qc = useQueryClient();
  const ms = useCountdown(round.deadline_at, party.clock);
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: partyKeys.party(party.id) }));
  return ms;
}

function Writing({ party, round, said, top, late }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode; late: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const ms = usePhaseClock(party, round);
  const byId = usePlayers(party);
  const mine = said.my_answer;
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(mine?.text ?? '');
  const [error, setError] = useState<string | null>(null);
  const send = useMutation({
    meta: { quiet: true },
    mutationFn: (answer: string) => partyApi.answer(party.id, answer),
    onMutate: () => setError(null),
    onSuccess: (p) => {
      setParty(p);
      setEditing(false);
    },
    onError: (e) => {
      // Too late (the guessing started, time is up, the party ended): the refetch moves the screen on.
      setError(partyError(e));
      qc.invalidateQueries({ queryKey: partyKeys.party(party.id) });
    },
  });
  const ok = !late && text.trim().length > 0 && !send.isPending && text.trim() !== mine?.text;
  const submit = () => ok && send.mutate(text.trim());
  const writing = !late && (!mine || editing);
  const writers = said.writer_ids.filter((id) => round.eligible.includes(id));
  return (
    <Screen
      tabs={false}
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
      <Countdown ms={ms} total={phaseSeconds(round, party.timer)} />
      {late ? (
        <LateCard />
      ) : writing || !mine ? (
        <AnswerField value={text} onChange={setText} onSubmit={submit} autoFocus={editing} error={error} />
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
            {tr('said.party.next')}
          </Text>
          <ErrorText>{error}</ErrorText>
        </>
      )}
      <VotedLine round={round} byId={byId} done={writers} label="said.party.written" />
    </Screen>
  );
}

function Guessing({ party, round, said, top }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const ms = usePhaseClock(party, round);
  const byId = usePlayers(party);
  const me = party.me?.member_id;
  const answers = useMemo(() => toGuess(said), [said]);
  // The writers but me (players are stable objects between refetches, so the chips only redraw when their state changes).
  const writers = said.writer_ids;
  const people = useMemo(
    () =>
      suspectIds({ writer_ids: writers }, me).flatMap((id) => {
        const m = byId.get(id);
        return m ? [m] : [];
      }),
    [writers, me, byId],
  );
  const [state, dispatch] = useMatchGame(answers.map((a) => a.id));
  const complete = isComplete(state);
  const nothing = answers.length === 0;
  const [error, setError] = useState<string | null>(null);
  const vote = useMutation({
    meta: { quiet: true },
    mutationFn: (guesses: SaidGuess[]) => partyApi.guess(party.id, guesses),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => {
      setError(partyError(e));
      qc.invalidateQueries({ queryKey: partyKeys.party(party.id) });
    },
  });
  const send = () => (complete || nothing) && vote.mutate(toGuesses(state));
  const bump = usePop(complete, 1.04, complete);
  // Reporting an answer needs an account (a guest can't report) and the moderation feature.
  const { moderation } = useFeatures();
  const canReport = moderation && !party.me?.guest;
  const [toReport, setToReport] = useState<ModTarget | null>(null);
  const onReport = useCallback((id: number) => setToReport({ kind: 'answer', id }), []);
  const done = placed(state);
  return (
    <Screen
      tabs={false}
      footer={
        <>
          <Countdown ms={ms} total={phaseSeconds(round, party.timer)} />
          {!nothing && <SuspectTray people={people} state={state} dispatch={dispatch} />}
          <ErrorText>{error}</ErrorText>
          <Animated.View style={bump}>
            <Button disabled={!complete && !nothing} loading={vote.isPending} onPress={send}>
              {complete || nothing ? tr('said.guess.submit') : tr('said.guess.progress', { done, total: answers.length })}
            </Button>
          </Animated.View>
        </>
      }>
      {top}
      {round.voted.length > 0 && <VotedLine round={round} byId={byId} label="said.party.guessed" />}
      {nothing ? (
        <Card style={{ padding: 16 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 21, color: t.mut }}>{tr('said.guess.nothing')}</Text>
        </Card>
      ) : (
        <>
          <ZesteSays seed={round.id} done={done} total={answers.length} />
          <Tip id="said" text={tr('said.guess.tip')} />
          <MatchBoard answers={answers} state={state} byId={byId} dispatch={dispatch} onReport={canReport ? onReport : undefined} />
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
      {canReport && <ModerationDialog target={toReport} onClose={() => setToReport(null)} />}
    </Screen>
  );
}

/** Guesses sent (or arrived too late to play this one): the clock, who has guessed, and what I said. */
function Guessed({ party, round, said, top, late }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode; late: boolean }) {
  const t = useTheme();
  const tr = useT();
  const ms = usePhaseClock(party, round);
  const byId = usePlayers(party);
  const answers = useMemo(() => toGuess(said), [said]);
  return (
    <Screen tabs={false}>
      {top}
      <Countdown ms={ms} total={phaseSeconds(round, party.timer)} />
      <Animated.View entering={motion.enter} style={{ gap: 16 }}>
        {late ? (
          <LateCard />
        ) : (
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <Check size={20} color={t.accInk} strokeWidth={3} />
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
              {tr('said.party.sent')}
            </Text>
          </Card>
        )}
        <VotedLine round={round} byId={byId} label="said.party.guessed" />
        {!late && <GuessRecap answers={answers} guesses={said.my_guesses ?? []} byId={byId} />}
        {!late && said.my_answer && <MyAnswer text={said.my_answer.text} />}
      </Animated.View>
    </Screen>
  );
}

function LateCard() {
  const t = useTheme();
  const tr = useT();
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
      <Hourglass size={20} color={t.accInk} />
      <Text numberOfLines={2} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
        {tr('party.vote.late')}
      </Text>
    </Card>
  );
}

/** The said result on a phone: the reveal (tap through it) then the scoreboard, or why there's none. */
export function PartySaidResult({ round, said, byId, me }: { round: PartyRound; said: SaidView; byId: Map<number, Member>; me: number | undefined }) {
  if (!said.result) return <TooFew size={72} />;
  return <SaidResultView roundId={round.id} said={said} byId={byId} me={me} />;
}

/** The question closed with fewer than two answers: nothing to guess. */
function TooFew({ size, big }: { size: number; big?: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.enter} style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
      <Mascot who="roupi" mood="sleepy" size={size} />
      <Text
        style={{
          flex: 1,
          fontFamily: big ? t.font.display : t.font.semibold,
          fontSize: big ? Math.round(size * 0.3) : 16,
          lineHeight: big ? Math.round(size * 0.36) : 22,
          color: big ? t.txt : t.mut,
        }}>
        {tr('said.result.tooFew')}
      </Text>
    </Animated.View>
  );
}

// ---------------------------------------------------------------------------------------------------------------------
// The big screen

/** "#3 · Qui a dit ça ?" over the question. */
function KindTag({ round }: { round: PartyRound }) {
  const tr = useT();
  return <Tag label={`#${round.number} · ${tr('said.reveal.title')}`} />;
}

/** The big screen while a said question is open: the prompt while people write, the answers while they guess. */
export function SaidScreenQuestion({ party, round, said, unit, wide }: { party: Party; round: PartyRound; said: SaidView; unit: number; wide: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const ms = useCountdown(round.deadline_at, party.clock);
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: partyKeys.screen(party.code) }));
  const byId = usePlayers(party);
  const total = phaseSeconds(round, party.timer);
  const big = (n: number) => Math.round(n * unit);
  if (said.phase === 'writing') {
    const writers = said.writer_ids.filter((id) => round.eligible.includes(id));
    return (
      <Animated.View entering={motion.question} style={{ gap: 32 * unit }}>
        <KindTag round={round} />
        <Display size={big(round.question.length > 90 ? 64 : 84)}>{tidy(round.question)}</Display>
        <Text style={{ fontFamily: t.font.semibold, fontSize: big(30), lineHeight: big(40), color: t.mut }}>{tr('said.party.write')}</Text>
        <Countdown ms={ms} total={total} big />
        <VotedLine round={round} byId={byId} done={writers} label="said.party.written" big />
      </Animated.View>
    );
  }
  return <ScreenAnswers round={round} said={said} byId={byId} ms={ms} total={total} unit={unit} wide={wide} />;
}

/** The guessing: every answer big and numbered (anonymous), who has guessed, the clock. */
function ScreenAnswers({
  round,
  said,
  byId,
  ms,
  total,
  unit,
  wide,
}: {
  round: PartyRound;
  said: SaidView;
  byId: Map<number, Member>;
  ms: number | null;
  total: number;
  unit: number;
  wide: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  const answers = useMemo(() => toGuess(said), [said]);
  const cols = wide && answers.length > 3 ? 2 : 1;
  const half = Math.ceil(answers.length / 2);
  // Fewer, longer answers get bigger type.
  const size = Math.round((answers.length > 6 ? 28 : answers.length > 3 ? 34 : 42) * unit);
  return (
    <Animated.View entering={motion.question} style={{ gap: 28 * unit }}>
      <View style={{ gap: 14 * unit }}>
        <KindTag round={round} />
        <Text numberOfLines={2} style={{ fontFamily: t.font.bold, fontSize: Math.round(36 * unit), lineHeight: Math.round(44 * unit), color: t.txt }}>
          {tidy(round.question)}
        </Text>
      </View>
      {/* Two columns on a wide screen: 1, 2, 3 down the left, the rest down the right. */}
      <View style={{ flexDirection: 'row', gap: 16 * unit }}>
        {(cols === 2 ? [answers.slice(0, half), answers.slice(half)] : [answers]).map((col, c) => (
          <View key={c} style={{ flex: 1, gap: 16 * unit }}>
            {col.map((a, i) => {
              const n = c * half + i + 1;
              return <ScreenAnswer key={a.id} answer={a} n={n} total={answers.length} size={size} />;
            })}
          </View>
        ))}
      </View>
      <View style={{ flexDirection: wide ? 'row' : 'column', alignItems: wide ? 'center' : 'stretch', gap: 32 * unit }}>
        <View style={{ flex: wide ? 1 : undefined, gap: 10 * unit }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: Math.round(24 * unit), color: t.mut }}>{tr('said.party.guess')}</Text>
          <VotedLine round={round} byId={byId} label="said.party.guessed" big />
        </View>
        <View style={{ flex: wide ? 1 : undefined }}>
          <Countdown ms={ms} total={total} big />
        </View>
      </View>
    </Animated.View>
  );
}

const ScreenAnswer = memo(function ScreenAnswer({
  answer,
  n,
  total,
  size,
}: {
  answer: SaidAnswer;
  n: number;
  total: number;
  size: number;
}) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View
      entering={motion.stagger(n - 1)}
      accessible
      accessibilityLabel={tr('said.guess.a11y', { n, total, text: answer.text ?? tr('said.guess.hidden') })}
      style={[
        {
          flexDirection: 'row',
          alignItems: 'flex-start',
          gap: Math.round(size * 0.6),
          padding: Math.round(size * 0.75),
          borderRadius: t.radius.card,
          backgroundColor: t.card,
        },
        t.shadow,
      ]}>
      <NumberDisc n={n} size={Math.round(size * 1.5)} />
      <View style={{ flex: 1, paddingTop: Math.round(size * 0.12) }}>
        <AnswerText answer={answer} size={size} />
      </View>
    </Animated.View>
  );
});

/** The big screen's said result: the authors one by one, then the scoreboard (and the reactions coming in). */
export function SaidScreenResult({
  party,
  round,
  said,
  unit,
  wide,
  reactions,
}: {
  party: Party;
  round: PartyRound;
  said: SaidView;
  unit: number;
  wide: boolean;
  reactions: boolean;
}) {
  const t = useTheme();
  const byId = usePlayers(party);
  const items = useMemo(() => revealItems(said, undefined), [said]);
  const steps = useMemo(() => screenRevealSteps(items), [items]);
  const step = useScreenStep(round.closed_at, party.clock, steps);
  const result = said.result;
  return (
    <View style={{ gap: 32 * unit }}>
      <View style={{ gap: 14 * unit }}>
        <KindTag round={round} />
        <Text numberOfLines={2} style={{ fontFamily: t.font.bold, fontSize: Math.round(34 * unit), lineHeight: Math.round(42 * unit), color: t.txt }}>
          {tidy(round.question)}
        </Text>
      </View>
      {!result ? (
        <TooFew size={Math.round(160 * unit)} big />
      ) : step < items.length ? (
        <SaidScreenReveal items={items} step={step} byId={byId} k={unit} wide={wide} />
      ) : (
        <Animated.View entering={motion.enter} style={{ gap: 28 * unit }}>
          <SaidScoreboard scores={result.scores} byId={byId} me={undefined} k={Math.max(1, 1.8 * unit)} columns={wide && result.scores.length > 4 ? 2 : 1} />
          {reactions ? <ScreenCounts reactions={round.reactions} unit={unit} /> : !!round.reactions?.length && <Reactions round={round} disabled />}
        </Animated.View>
      )}
    </View>
  );
}
