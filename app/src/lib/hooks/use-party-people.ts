import { type Member, useMembers } from '@/lib/api/group';
import type { Party } from '@/lib/api/party';

/** The players, plus people who left since (their votes and wins keep a face). */
export function usePartyPeople(party: Party) {
  const { byId } = useMembers();
  const all = new Map<number, Member>(byId);
  for (const m of party.players) all.set(m.id, m);
  return all;
}
