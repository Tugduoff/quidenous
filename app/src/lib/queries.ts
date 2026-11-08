import { useQuery } from '@tanstack/react-query';

import { api, type HistoryItem, type Member, type Queue, type Round } from './api';

export const keys = {
  group: ['group'] as const,
  current: ['round', 'current'] as const,
  unseen: ['round', 'unseen'] as const,
  round: (id: number) => ['round', id] as const,
  history: ['history'] as const,
  queue: ['queue'] as const,
};

export const useGroup = () =>
  useQuery({ queryKey: keys.group, queryFn: () => api<{ name: string; members: Member[] }>('/group') });

export const useCurrentRound = () =>
  useQuery({ queryKey: keys.current, queryFn: () => api<Round | null>('/round/current') });

/** A finished question I voted on but have not seen the result of (I was away when it closed). */
export const useUnseenRound = () =>
  useQuery({ queryKey: keys.unseen, queryFn: () => api<Round | null>('/round/unseen') });

export const useRound = (id: number | null) =>
  useQuery({ queryKey: keys.round(id ?? 0), queryFn: () => api<Round>(`/rounds/${id}`), enabled: id != null });

export const useHistory = () => useQuery({ queryKey: keys.history, queryFn: () => api<HistoryItem[]>('/history') });

export const useQueue = () => useQuery({ queryKey: keys.queue, queryFn: () => api<Queue>('/questions/queue') });

/** id → member, for avatars everywhere. */
export function useMembers() {
  const { data } = useGroup();
  const byId = new Map((data?.members ?? []).map((m) => [m.id, m]));
  return { members: data?.members ?? [], byId };
}
