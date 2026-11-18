// « Qui a dit ça ? » (docs/SAID.md): everyone writes an answer, then matches the answers to their authors, then the reveal.
//
// The pieces (AnswerField, MyAnswer, MatchBoard + SuspectTray, GuessRecap, SaidReveal, SaidAnswerList, SaidScoreboard,
// SaidResultView) only take props: no room query in them, so a party screen can reuse them. `SaidRound` is the room's
// open question (it talks to `/round/current/*`).
import { type ReactNode, useState } from 'react';
import Animated, { useReducedMotion } from 'react-native-reanimated';
import { AgainButton } from '@/components/said/again-button';
import { GuessRecap } from '@/components/said/guess-recap';
import { MyAnswer } from '@/components/said/my-answer';
import { SaidAnswerList } from '@/components/said/said-answer-list';
import { SaidGuessing } from '@/components/said/said-guessing';
import { SaidReveal } from '@/components/said/said-reveal';
import { SaidScoreboard } from '@/components/said/said-scoreboard';
import { SaidWriting } from '@/components/said/said-writing';
import { Screen } from '@/components/ui/screen';
import type { Member } from '@/lib/api/group';
import type { Round, SaidView } from '@/lib/api/rounds';
import { markSaidSeen, useSaidSeen } from '@/lib/hooks/use-said-seen';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { revealItems, toGuess } from '@/lib/utils/said';

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

/**
 * A finished said question: the reveal the first time (on this device), then the scoreboard and every answer with its
 * author. Reduced motion: straight to the full result.
 */
export function SaidResultView({ roundId, said, byId, me }: { roundId: number; said: SaidView; byId: Map<number, Member>; me: number | undefined }) {
  const tr = useT();
  const reduced = useReducedMotion();
  const seen = useSaidSeen(roundId);
  const [replay, setReplay] = useState(false);
  const items = revealItems(said, me);
  if (!said.result || seen == null) return null;
  if ((!seen || replay) && !reduced && items.length > 0)
    return (
      <SaidReveal
        items={items}
        byId={byId}
        onDone={() => {
          markSaidSeen(roundId);
          setReplay(false);
        }}
      />
    );
  return (
    <Animated.View entering={motion.enter} style={{ gap: 20 }}>
      <SaidScoreboard scores={said.result.scores} byId={byId} me={me} />
      {items.length > 0 && <SaidAnswerList items={items} byId={byId} />}
      {!reduced && items.length > 0 && (
        <AgainButton label={tr('said.reveal.replay')} onPress={() => setReplay(true)} />
      )}
    </Animated.View>
  );
}
