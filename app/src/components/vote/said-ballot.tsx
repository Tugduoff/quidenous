import { SaidRound } from '@/components/said/said';
import { useSession } from '@/lib/session';
import { type Member } from '@/lib/api/group';
import { type Round, type SaidView } from '@/lib/api/rounds';
import { Question } from '@/components/vote/question';
import { Header } from '@/components/vote/header';
import { WhoIsIn } from '@/components/vote/who-is-in';

export function SaidBallot({ round, said, members, byId, onVoted }: { round: Round; said: SaidView; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  const { me } = useSession();
  return (
    <SaidRound
      round={round}
      said={said}
      me={me?.id}
      byId={byId}
      onVoted={onVoted}
      top={
        <>
          <Header />
          <Question round={round} />
        </>
      }
      // While writing, "done" is having written; then, having guessed.
      waitList={(done, title) => <WhoIsIn round={round} members={members} byId={byId} done={done} title={title} />}
    />
  );
}
