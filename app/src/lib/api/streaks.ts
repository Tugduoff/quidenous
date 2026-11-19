import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { useGroup } from '@/lib/api/group';
import { roundKeys } from '@/lib/api/rounds';
import { useFeatures } from '@/lib/api/features';

type Streak = { current: number; best: number };
export type Streaks = { members: Record<string, Streak>; room: Streak | null };

const streakKeys = {
  room: [...roundKeys.current, 'streaks'] as const,
};

export function useStreaks() {
  const { streaks } = useFeatures();
  const { data: group } = useGroup();
  const on = streaks && !!group && group.kind !== 'party';
  const query = useQuery({ queryKey: streakKeys.room, queryFn: () => api<Streaks>('/streaks'), enabled: on, staleTime: 60_000 });
  return on ? query.data : undefined;
}

export function useMemberStreak(id: number | null | undefined) {
  const data = useStreaks();
  return id == null ? 0 : (data?.members[String(id)]?.current ?? 0);
}
