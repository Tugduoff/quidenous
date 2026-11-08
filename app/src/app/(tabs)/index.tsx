import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowRight, Check, Hourglass } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { ResultBars } from '@/components/result-bars';
import { Screen } from '@/components/screen';
import { Avatar, AvatarStack, Button, Card, Display, Logo, Tag } from '@/components/ui';
import { api, type Member, type Round } from '@/lib/api';
import { keys, useCurrentRound, useMembers, useRound } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

function Header() {
  const { me } = useSession();
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}>
      <Logo />
      <Avatar member={me ?? undefined} size={36} />
    </View>
  );
}

export default function VoteScreen() {
  const t = useTheme();
  const { me } = useSession();
  const { byId, members } = useMembers();
  const current = useCurrentRound();
  // The round I voted in: once it closes, show its result until "next".
  const [watching, setWatching] = useState<number | null>(null);
  const round = current.data;

  useEffect(() => {
    if (round?.my_vote != null) setWatching(round.id);
  }, [round?.id, round?.my_vote]);

  const showResult = watching != null && round?.id !== watching;
  if (showResult) return <Result roundId={watching} byId={byId} onNext={() => setWatching(null)} />;
  if (!round) {
    return (
      <Screen>
        <Header />
        <Display>{current.isLoading ? '' : 'Plus de questions. Ajoutez-en !'}</Display>
      </Screen>
    );
  }
  if (round.my_vote != null)
    return <Waiting round={round} byId={byId} members={members} />;
  return (
    <Ballot
      key={round.id}
      round={round}
      members={members}
      byId={byId}
      onVoted={() => setWatching(round.id)}
    />
  );
}

function Question({ round, result }: { round: Round; result?: boolean }) {
  return (
    <View style={{ gap: 14 }}>
      <Tag label={`${result ? 'Résultat' : round.custom ? 'De nous' : 'Dilemme'} · #${round.number}`} tone={result ? 'acc' : 'tag'} />
      <Display size={result ? 28 : 34}>{round.question}</Display>
    </View>
  );
}

function Ballot({
  round,
  members,
  byId,
  onVoted,
}: {
  round: Round;
  members: Member[];
  byId: Map<number, Member>;
  onVoted: () => void;
}) {
  const t = useTheme();
  const qc = useQueryClient();
  const [pick, setPick] = useState<number | null>(null);
  const vote = useMutation({
    mutationFn: (target_id: number) => api('/round/current/vote', { method: 'POST', json: { target_id } }),
    // Remember the round right away: when my vote is the last one, the next question
    // replaces it before the refetch could show my vote, and the result must still show.
    onSuccess: onVoted,
    onSettled: () => qc.invalidateQueries({ queryKey: keys.current }),
  });
  const picked = pick != null ? byId.get(pick) : undefined;
  return (
    <Screen
      footer={
        <Button disabled={!picked} loading={vote.isPending} onPress={() => pick != null && vote.mutate(pick)}>
          {picked ? `Voter pour ${picked.name}` : `${round.voter_ids.length}/${round.member_count}`}
        </Button>
      }>
      <Header />
      <Question round={round} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} />
        <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
          {round.voter_ids.length}/{round.member_count}
        </Text>
      </View>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
        {members.map((m) => {
          const on = m.id === pick;
          return (
            <Pressable
              key={m.id}
              accessibilityRole="radio"
              accessibilityState={{ selected: on }}
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
              <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt, flex: 1 }} numberOfLines={1}>
                {m.name}
              </Text>
            </Pressable>
          );
        })}
      </View>
    </Screen>
  );
}

function Waiting({ round, byId, members }: { round: Round; byId: Map<number, Member>; members: Member[] }) {
  const t = useTheme();
  const target = round.my_vote != null ? byId.get(round.my_vote) : undefined;
  const pending = members.filter((m) => !round.voter_ids.includes(m.id));
  return (
    <Screen footer={<Button disabled>{`${round.voter_ids.length}/${round.member_count}`}</Button>}>
      <Header />
      <Question round={round} />
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 14, padding: 20 }}>
        <Avatar member={target} size={56} />
        <Text style={{ flex: 1, fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{target?.name}</Text>
        <View style={{ width: 36, height: 36, borderRadius: 18, backgroundColor: t.acc, alignItems: 'center', justifyContent: 'center' }}>
          <Check size={20} color={t.accTxt} />
        </View>
      </Card>
      {pending.map((m) => (
        <View key={m.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <Avatar member={m} size={32} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, color: t.txt }}>{m.name}</Text>
          <Hourglass size={18} color={t.mut} />
        </View>
      ))}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} size={32} />
        <Check size={18} color={t.accInk} />
      </View>
    </Screen>
  );
}

export function Result({ roundId, byId, onNext }: { roundId: number; byId: Map<number, Member>; onNext?: () => void }) {
  const t = useTheme();
  const { data: round } = useRound(roundId);
  if (!round?.result) return <Screen><Header /></Screen>;
  return (
    <Screen
      footer={
        onNext && (
          <Button onPress={onNext}>
            <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>Suivante</Text>
            <ArrowRight size={20} color={t.accTxt} />
          </Button>
        )
      }>
      <Header />
      <Question round={round} result />
      <ResultBars result={round.result} byId={byId} />
    </Screen>
  );
}
