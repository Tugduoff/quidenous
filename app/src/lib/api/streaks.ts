import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { useGroup } from '@/lib/api/group';
import { roundKeys } from '@/lib/api/rounds';
import { useFeatures } from '@/lib/api/features';

/** Days in a row with a vote (docs/STREAKS.md): `current` (today counts once voted) and the best ever. */
type Streak = { current: number; best: number };
/** `GET /streaks`: per current member (by id) and for the whole room; a party has none. */
export type Streaks = { members: Record<string, Streak>; room: Streak | null };

/** Under the open question's key: every vote, new question or arrival (live events) refreshes it with it. */
const streakKeys = {
  room: [...roundKeys.current, 'streaks'] as const,
};

/** The room's streaks, from a server that has them (`streaks` in /api/features); never in a party. */
export function useStreaks() {
  const { streaks } = useFeatures();
  const { data: group } = useGroup();
  const on = streaks && !!group && group.kind !== 'party';
  const query = useQuery({ queryKey: streakKeys.room, queryFn: () => api<Streaks>('/streaks'), enabled: on, staleTime: 60_000 });
  return on ? query.data : undefined;
}

/** Someone's current streak in this room (0 when unknown). */
export function useMemberStreak(id: number | null | undefined) {
  const data = useStreaks();
  return id == null ? 0 : (data?.members[String(id)]?.current ?? 0);
}
