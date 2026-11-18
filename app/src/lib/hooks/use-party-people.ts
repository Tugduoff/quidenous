import { type Member, useMembers } from '@/lib/api/group';
import type { Party } from '@/lib/api/party';

export function usePartyPeople(party: Party) {
  const { byId } = useMembers();
  const all = new Map<number, Member>(byId);
  for (const m of party.players) all.set(m.id, m);
  return all;
}
