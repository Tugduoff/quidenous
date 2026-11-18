import type { Member } from '@/lib/api/group';
import type { Party, PartyRound } from '@/lib/api/party';
import type { Tally } from '@/lib/api/rounds';

/**
 * The current phase's length in seconds, for the countdown's bar: a pick question's timer; a said question's writing
 * (`max(60, 2 × timer)`), then its guessing (`max(45, 2 × timer)`), read from the deadlines when they're there.
 */
export function phaseSeconds(round: Pick<PartyRound, 'opened_at' | 'deadline_at' | 'said'>, timer: number): number {
  const said = round.said;
  const fallback = !said ? timer : said.phase === 'guessing' ? Math.max(45, 2 * timer) : Math.max(60, 2 * timer);
  const start = said?.phase === 'guessing' ? said.guess_at : round.opened_at;
  const s = (Date.parse(round.deadline_at ?? '') - Date.parse(start ?? '')) / 1000;
  return Number.isFinite(s) && s > 0 ? Math.round(s) : fallback;
}

/** A result as the shared result bars draw it. */
export const tallies = (round: PartyRound): Tally[] =>
  (round.results ?? []).map((r) => ({ target_id: r.member_id, count: r.votes, voter_ids: r.voters })).sort((a, b) => b.count - a.count);

export function stepAt(closedAt: string | null, clock: number, steps: number[], now: number): { step: number; left: number | null } {
  const start = Date.parse(closedAt ?? '');
  let elapsed = Number.isFinite(start) ? Math.max(0, now + clock - start) : 0;
  for (let i = 0; i < steps.length; i++) {
    if (elapsed < steps[i]) return { step: i, left: steps[i] - elapsed };
    elapsed -= steps[i];
  }
  return { step: steps.length, left: null };
}

/** The party's players by id. */
export const playersById = (p: Pick<Party, 'players'>) => new Map<number, Member>(p.players.map((m) => [m.id, m]));

export const isPartyOwner = (p: Pick<Party, 'me'>) => p.me?.role === 'owner';
