import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Check, Hourglass } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Countdown, VotedLine } from '@/components/party/party';
import { PartyHeader } from '@/components/party/party-header';
import { PartyQuestion } from '@/components/party/party-question';
import { Avatar } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Screen } from '@/components/ui/screen';
import { ErrorText } from '@/components/ui/text';
import { BIG_GROUP, gridColumns, MemberGrid, MemberTile } from '@/components/vote/member-grid';
import type { Member } from '@/lib/api/group';
import { type Party, partyApi, partyError, partyKeys, type PartyRound, useSetParty } from '@/lib/api/party';
import { useCountdown } from '@/lib/hooks/use-countdown';
import { useRefetchAtZero } from '@/lib/hooks/use-refetch-at-zero';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { playersById } from '@/lib/utils/party';

/** The open question: countdown, pick someone, then who has voted. Someone who arrived after it opened watches. */
export function PartyVoting({ party, round }: { party: Party; round: PartyRound }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const ms = useCountdown(round.deadline_at, party.clock);
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: partyKeys.party(party.id) }));
  const [pick, setPick] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const byId = playersById(party);
  const me = party.me?.member_id;
  const late = me == null || !round.eligible.includes(me);
  const voted = round.my_vote != null || (me != null && round.voted.includes(me));
  const vote = useMutation({
    meta: { quiet: true },
    mutationFn: (target: number) => partyApi.vote(party.id, target),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => {
      setError(partyError(e));
      qc.invalidateQueries({ queryKey: partyKeys.party(party.id) });
    },
  });
  const choices = round.eligible.map((pid) => byId.get(pid)).filter((m): m is Member => !!m);
  const picked = pick != null ? byId.get(pick) : undefined;
  const mine = round.my_vote != null ? byId.get(round.my_vote) : undefined;
  return (
    <Screen
      tabs={false}
      footer={
        !late &&
        !voted && (
          <>
            <ErrorText>{error}</ErrorText>
            <Button disabled={!picked} loading={vote.isPending} onPress={() => pick != null && vote.mutate(pick)}>
              {picked ? (
                <Text numberOfLines={1} style={{ flexShrink: 1, paddingHorizontal: 20, fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>
                  {tr('party.vote.voteFor', { name: picked.name })}
                </Text>
              ) : (
                tr('party.vote.pick')
              )}
            </Button>
          </>
        )
      }>
      <PartyHeader party={party} />
      <PartyQuestion round={round} size={32} />
      <Countdown ms={ms} total={party.timer} />
      {late || voted ? (
        <Animated.View entering={motion.enter} style={{ gap: 16 }}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            {late ? <Hourglass size={20} color={t.accInk} /> : <Avatar member={mine} size={36} />}
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
              {late ? tr('party.vote.late') : mine ? tr('party.vote.myVote', { name: mine.name }) : tr('party.vote.done')}
            </Text>
            {!late && <Check size={20} color={t.accInk} />}
          </Card>
          <VotedLine round={round} byId={byId} />
        </Animated.View>
      ) : (
        <>
          {round.voted.length > 0 && <VotedLine round={round} byId={byId} />}
          {choices.length > BIG_GROUP ? (
            <MemberGrid>
              {choices.map((m) => (
                <MemberTile
                  key={m.id}
                  member={m}
                  cols={gridColumns(choices.length)}
                  role="radio"
                  selected={m.id === pick}
                  onPress={() => setPick(m.id)}
                />
              ))}
            </MemberGrid>
          ) : (
            <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
              {choices.map((m) => {
                const on = m.id === pick;
                return (
                  <PressableScale
                    key={m.id}
                    scale={0.97}
                    accessibilityRole="radio"
                    accessibilityState={{ selected: on }}
                    accessibilityLabel={m.name}
                    onPress={() => setPick(m.id)}
                    style={[
                      {
                        width: '48%',
                        flexDirection: 'row',
                        alignItems: 'center',
                        gap: 10,
                        padding: 14,
                        borderRadius: 20,
                        borderWidth: 2,
                        borderColor: on ? t.accInk : 'transparent',
                        backgroundColor: on ? t.accSoft : t.card,
                      },
                      t.shadow,
                    ]}>
                    <Avatar member={m} />
                    <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
                      {m.name}
                    </Text>
                  </PressableScale>
                );
              })}
            </View>
          )}
        </>
      )}
    </Screen>
  );
}
