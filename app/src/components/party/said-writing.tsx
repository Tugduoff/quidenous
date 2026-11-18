import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type ReactNode, useState } from 'react';
import { Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { LateCard } from '@/components/party/late-card';
import { Countdown, VotedLine } from '@/components/party/party';
import { AnswerButtons } from '@/components/said/answer-buttons';
import { AnswerField } from '@/components/said/answer-field';
import { MyAnswer } from '@/components/said/my-answer';
import { Screen } from '@/components/ui/screen';
import { ErrorText } from '@/components/ui/text';
import { type Party, partyApi, partyError, partyKeys, type PartyRound, useSetParty } from '@/lib/api/party';
import type { SaidView } from '@/lib/api/rounds';
import { usePartyPlayers } from '@/lib/hooks/use-party-players';
import { usePhaseClock } from '@/lib/hooks/use-phase-clock';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { phaseSeconds } from '@/lib/utils/party';

export function PartySaidWriting({ party, round, said, top, late }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode; late: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const ms = usePhaseClock(party, round);
  const byId = usePartyPlayers(party);
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
