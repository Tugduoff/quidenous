import { useQueryClient } from '@tanstack/react-query';
import { Redirect } from 'expo-router';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { NextGate } from '@/components/vote/next-gate';
import { NoQuestions, SoloInvite } from '@/components/vote/vote-empty';
import { useLandOnToday } from '@/lib/hooks/use-today-tab';
import { useGroup, useMembers } from '@/lib/api/group';
import { useCurrentRound, useUnseenRound, roundKeys, markRoundSeen } from '@/lib/api/rounds';
import { useRooms } from '@/lib/api/rooms';
import { Result } from '@/components/result/result';
import { Header } from '@/components/vote/header';
import { Ballot } from '@/components/vote/ballot';
import { SaidBallot } from '@/components/vote/said-ballot';
import { Waiting } from '@/components/vote/waiting';

export default function VoteScreen() {
  const { byId, members } = useMembers();
  const current = useCurrentRound();
  const unseen = useUnseenRound();
  const qc = useQueryClient();
  const group = useGroup();
  const round = current.data;
  const pace = group.data?.pace;
  const held = !round && pace?.waiting && pace.round_id ? pace.round_id : null;
  const { data: rooms } = useRooms();
  const land = useLandOnToday();
  if (land) return <Redirect href="/today" />;
  const roomId = group.data?.id;
  if (roomId != null && (group.data?.kind === 'party' || rooms?.find((r) => r.id === roomId)?.kind === 'party'))
    return <Redirect href={`/party/${roomId}`} />;

  if (unseen.data) {
    const seenUpTo = (id: number) => markRoundSeen(id).finally(() => qc.invalidateQueries({ queryKey: roundKeys.unseen }));
    const unseenId = unseen.data.id;
    const seen = () => seenUpTo(unseenId);
    const gate = held === unseen.data.id && group.data ? <NextGate group={group.data} onLaunched={seen} /> : undefined;
    const { unseen_count: count = 1, unseen_last_id: last } = unseen.data;
    const skip = !gate && count >= 2 && last != null ? { count, onPress: () => seenUpTo(last) } : undefined;
    return <Result key={unseen.data.id} roundId={unseen.data.id} byId={byId} onNext={seen} gate={gate} skip={skip} />;
  }
  if (group.data && group.data.members.length < 2) {
    return (
      <Screen>
        <Header />
        <SoloInvite />
      </Screen>
    );
  }
  if (held && group.data) return <Result key={held} roundId={held} byId={byId} gate={<NextGate group={group.data} />} />;
  if (!round) {
    return (
      <Screen>
        <Header />
        {current.isError || group.isError ? (
          <QueryState query={current.isError ? current : group} />
        ) : (
          !current.isLoading && !group.isLoading && <NoQuestions />
        )}
      </Screen>
    );
  }
  if (round.kind === 'said' && round.said)
    return <SaidBallot key={round.id} round={round} said={round.said} members={members} byId={byId} onVoted={() => qc.invalidateQueries({ queryKey: roundKeys.unseen })} />;
  if (round.my_vote != null) return <Waiting round={round} byId={byId} members={members} />;
  return <Ballot key={round.id} round={round} members={members} byId={byId} onVoted={() => qc.invalidateQueries({ queryKey: roundKeys.unseen })} />;
}
