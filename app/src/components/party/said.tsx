// « Qui a dit ça ? » at a party (docs/SAID.md, Party): the phone (write, guess, the result) and the big screen (the
// prompt while people write, the answers while they guess, the authors one by one, the scoreboard). Built from the
// room's pieces (components/said.tsx); the party's own calls are in lib/party.ts.
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

/** The answers to guess, in order: the board starts over only if that changes (an answer taken away by someone leaving). */
export const saidBoardKey = (said: SaidView) =>
  toGuess(said)
    .map((a) => a.id)
    .join(',');

/**
 * The open said question on a player's phone: write (and rewrite until the writing deadline), then match the answers to
 * their authors, then wait for the others. `top`: the header and the question. Keyed by the parent on round, phase and
 * board, so each phase starts fresh.
 */
export function PartySaidVoting({ party, round, said, top }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode }) {
  const me = party.me?.member_id;
  // Arrived after the question opened: plays from the next one.
  const late = me == null || !round.eligible.includes(me);
  if (said.phase === 'writing') return <PartySaidWriting party={party} round={round} said={said} top={top} late={late} />;
  const guessed = round.my_vote != null || (me != null && round.voted.includes(me));
  if (!late && !guessed) return <PartySaidGuessing party={party} round={round} said={said} top={top} />;
  return <PartySaidGuessed party={party} round={round} said={said} top={top} late={late} />;
}

/** The said result on a phone: the reveal (tap through it) then the scoreboard, or why there's none. */
export function PartySaidResult({ round, said, byId, me }: { round: PartyRound; said: SaidView; byId: Map<number, Member>; me: number | undefined }) {
  if (!said.result) return <SaidTooFew size={72} />;
  return <SaidResultView roundId={round.id} said={said} byId={byId} me={me} />;
}
