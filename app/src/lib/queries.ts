import { useInfiniteQuery, useQuery } from '@tanstack/react-query';

import { api, type Group, type HistoryItem, type Member, type Queue, type Round, type Stats } from './api';

export const keys = {
  group: ['group'] as const,
  current: ['round', 'current'] as const,
  unseen: ['round', 'unseen'] as const,
  round: (id: number) => ['round', id] as const,
  history: ['history'] as const,
  pinned: ['history', 'pinned'] as const,
  stats: ['stats'] as const,
  queue: ['queue'] as const,
};

export const useGroup = () =>
  useQuery({ queryKey: keys.group, queryFn: () => api<Group>('/group') });

export const useCurrentRound = () =>
  useQuery({ queryKey: keys.current, queryFn: () => api<Round | null>('/round/current') });

/** A finished question I voted on but have not seen the result of (I was away when it closed). */
export const useUnseenRound = () =>
  useQuery({ queryKey: keys.unseen, queryFn: () => api<Round | null>('/round/unseen') });

export const useRound = (id: number | null) =>
  useQuery({ queryKey: keys.round(id ?? 0), queryFn: () => api<Round>(`/rounds/${id}`), enabled: id != null });

/** Same size as `HISTORY_PAGE` on the server: a shorter page is the last one. */
const HISTORY_PAGE = 30;

/** Finished questions, newest first, a page at a time: `before` is the number of the oldest one shown. */
export const useHistory = () =>
  useInfiniteQuery({
    queryKey: keys.history,
    queryFn: ({ pageParam }) => api<HistoryItem[]>(pageParam == null ? '/history' : `/history?before=${pageParam}`),
    initialPageParam: null as number | null,
    getNextPageParam: (last) => (last.length < HISTORY_PAGE ? undefined : last.at(-1)!.number),
  });

/** Results pinned for the whole group (all of them, not paged). */
export const usePinned = () => useQuery({ queryKey: keys.pinned, queryFn: () => api<HistoryItem[]>('/history/pinned') });

export const useStats = () => useQuery({ queryKey: keys.stats, queryFn: () => api<Stats>('/stats') });

export const useQueue = () => useQuery({ queryKey: keys.queue, queryFn: () => api<Queue>('/questions/queue') });

/** id → member, for avatars everywhere. */
export function useMembers() {
  const { data } = useGroup();
  const byId = new Map((data?.members ?? []).map((m) => [m.id, m]));
  return { members: data?.members ?? [], byId };
}
