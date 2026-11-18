import { useMemo } from 'react';
import type { Member } from '@/lib/api/group';
import type { Party } from '@/lib/api/party';

/** The players by id. Kept while the players don't change (refetches share unchanged parts), so lists don't redraw. */
export function usePartyPlayers(party: Party) {
  const players = party.players;
  return useMemo(() => new Map<number, Member>(players.map((m) => [m.id, m])), [players]);
}
