import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { NextButton, Reactions, WinnerReveal } from '@/components/party/party';
import { PartyHeader } from '@/components/party/party-header';
import { PartySaidResult } from '@/components/party/said';
import { LiveReactions } from '@/components/result/live-reactions';
import { ResultBars } from '@/components/result/result-bars';
import { Screen } from '@/components/ui/screen';
import { Tag } from '@/components/ui/tag';
import { ErrorText } from '@/components/ui/text';
import { useFeatures } from '@/lib/api/features';
import { type Party, partyApi, partyError, partyKeys, type PartyRound, useSetParty } from '@/lib/api/party';
import { usePartyPeople } from '@/lib/hooks/use-party-people';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { isPartyOwner, tallies } from '@/lib/utils/party';
import { tidy } from '@/lib/utils/text';

export function PartyReveal({ party, round }: { party: Party; round: PartyRound }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const [error, setError] = useState<string | null>(null);
  const byId = usePartyPeople(party);
  const owner = isPartyOwner(party);
  const { resultReactions } = useFeatures();
  const refetch = () => qc.invalidateQueries({ queryKey: partyKeys.party(party.id) });
  const next = useMutation({
    meta: { quiet: true },
    mutationFn: (undo: boolean) => (undo ? partyApi.unnext(party.id) : partyApi.next(party.id)),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => {
      setError(partyError(e));
      refetch();
    },
  });
  const react = useMutation({
    mutationFn: (emoji: string) => partyApi.react(party.id, round.id, emoji),
    onSuccess: setParty,
    onError: refetch,
  });
  const list = tallies(round);
  const said = round.kind === 'said' ? round.said : null;
  return (
    <Screen
      tabs={false}
      footer={
        party.me && (
          <>
            <ErrorText>{error}</ErrorText>
            <NextButton round={round} owner={owner} busy={next.isPending} onPress={() => next.mutate(!owner && !!round.next?.mine)} />
          </>
        )
      }>
      <PartyHeader party={party} />
      <View style={{ gap: 8 }}>
        <Tag label={said ? `#${round.number} · ${tr('said.reveal.title')}` : `#${round.number}`} tone="acc" />
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, lineHeight: 24, color: t.txt }}>{tidy(round.question)}</Text>
      </View>
      {said ? (
        <PartySaidResult round={round} said={said} byId={byId} me={party.me?.member_id} />
      ) : (
        <>
          <View style={{ paddingVertical: 8 }}>
            <WinnerReveal round={round} byId={byId} size={112} />
          </View>
          {list.length > 0 && <ResultBars result={list} byId={byId} />}
        </>
      )}
      {resultReactions ? (
        party.me && <LiveReactions roundId={round.id} byId={byId} me={party.me.member_id} />
      ) : (
        <Reactions round={round} disabled={!party.me} onReact={(e) => react.mutate(e)} />
      )}
    </Screen>
  );
}
