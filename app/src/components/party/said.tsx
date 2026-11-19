import type { ReactNode } from 'react';
import { PartySaidGuessed } from '@/components/party/said-guessed';
import { PartySaidGuessing } from '@/components/party/said-guessing';
import { SaidTooFew } from '@/components/party/said-too-few';
import { PartySaidWriting } from '@/components/party/said-writing';
import { SaidResultView } from '@/components/said/said';
import type { Member } from '@/lib/api/group';
import type { Party, PartyRound } from '@/lib/api/party';
import type { SaidView } from '@/lib/api/rounds';
import { toGuess } from '@/lib/utils/said';

export const saidBoardKey = (said: SaidView) =>
  toGuess(said)
    .map((a) => a.id)
    .join(',');

export function PartySaidVoting({ party, round, said, top }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode }) {
  const me = party.me?.member_id;
  const late = me == null || !round.eligible.includes(me);
  if (said.phase === 'writing') return <PartySaidWriting party={party} round={round} said={said} top={top} late={late} />;
  const guessed = round.my_vote != null || (me != null && round.voted.includes(me));
  if (!late && !guessed) return <PartySaidGuessing party={party} round={round} said={said} top={top} />;
  return <PartySaidGuessed party={party} round={round} said={said} top={top} late={late} />;
}

export function PartySaidResult({ round, said, byId, me }: { round: PartyRound; said: SaidView; byId: Map<number, Member>; me: number | undefined }) {
  if (!said.result) return <SaidTooFew size={72} />;
  return <SaidResultView roundId={round.id} said={said} byId={byId} me={me} />;
}
