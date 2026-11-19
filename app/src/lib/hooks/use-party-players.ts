import { useMemo } from 'react';
import type { Member } from '@/lib/api/group';
import type { Party } from '@/lib/api/party';

export function usePartyPlayers(party: Party) {
  const players = party.players;
  return useMemo(() => new Map<number, Member>(players.map((m) => [m.id, m])), [players]);
}
