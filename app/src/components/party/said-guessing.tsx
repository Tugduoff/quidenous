import { useMutation, useQueryClient } from '@tanstack/react-query';
import { RotateCcw } from 'lucide-react-native';
import { type ReactNode, useCallback, useMemo, useState } from 'react';
import { Pressable, Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { Countdown, VotedLine } from '@/components/party/party';
import { MatchBoard } from '@/components/said/match-board';
import { SuspectTray } from '@/components/said/suspect-tray';
import { ZesteSays } from '@/components/said/zeste-says';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Screen } from '@/components/ui/screen';
import { Tip } from '@/components/ui/tip';
import { ErrorText } from '@/components/ui/text';
import { useFeatures } from '@/lib/api/features';
import { type Party, partyApi, partyError, partyKeys, type PartyRound, useSetParty } from '@/lib/api/party';
import type { SaidGuess, SaidView } from '@/lib/api/rounds';
import { useMatchGame } from '@/lib/hooks/use-match-game';
import { usePartyPlayers } from '@/lib/hooks/use-party-players';
import { usePhaseClock } from '@/lib/hooks/use-phase-clock';
import { useT } from '@/lib/i18n';
import { usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { phaseSeconds } from '@/lib/utils/party';
import { isComplete, placed, suspects as suspectIds, toGuess, toGuesses } from '@/lib/utils/said';

export function PartySaidGuessing({ party, round, said, top }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const ms = usePhaseClock(party, round);
  const byId = usePartyPlayers(party);
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
