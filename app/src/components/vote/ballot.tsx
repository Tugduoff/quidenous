import { ChooseBallot } from '@/components/choose/choose';
import { type Member } from '@/lib/api/group';
import { type Round } from '@/lib/api/rounds';
import { Question } from '@/components/vote/question';
import { Header } from '@/components/vote/header';
import { PickBallot } from '@/components/vote/pick-ballot';
import { RankBallot } from '@/components/vote/rank-ballot';

export function Ballot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  // Same component instance per round (keyed by id), so the kind never changes under it.
  if (round.kind === 'rank') return <RankBallot round={round} members={members} byId={byId} onVoted={onVoted} />;
  // A choose question without its items (shouldn't happen) falls back to a plain pick.
  if (round.kind === 'choose' && (round.items?.length ?? 0) >= 2)
    return (
      <ChooseBallot
        round={round}
        members={members}
        byId={byId}
        onVoted={onVoted}
        top={
          <>
            <Header />
            <Question round={round} />
          </>
        }
      />
    );
  return <PickBallot round={round} members={members} byId={byId} onVoted={onVoted} />;
}
