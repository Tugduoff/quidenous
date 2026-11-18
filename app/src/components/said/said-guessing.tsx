import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type ReactNode, useCallback, useMemo, useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AgainButton } from '@/components/said/again-button';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { MatchBoard } from '@/components/said/match-board';
import { SuspectTray } from '@/components/said/suspect-tray';
import { ZesteSays } from '@/components/said/zeste-says';
import { AvatarStack } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Screen } from '@/components/ui/screen';
import { Tip } from '@/components/ui/tip';
import { groupKeys, type Member } from '@/lib/api/group';
import { type Round, roundKeys, type SaidGuess, type SaidView, voteCurrent } from '@/lib/api/rounds';
import { useMatchGame } from '@/lib/hooks/use-match-game';
import { useT } from '@/lib/i18n';
import { usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { isComplete, placed, suspects as suspectIds, toGuess, toGuesses } from '@/lib/utils/said';

export function SaidGuessing({
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
            <AgainButton label={tr('said.guess.restart')} onPress={() => dispatch({ type: 'reset' })} />
          )}
        </>
      )}
      <ModerationDialog target={toReport} onClose={() => setToReport(null)} />
    </Screen>
  );
}
