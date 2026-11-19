import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { featuresQuery } from '@/lib/api/features';

export type GlobalCount = { target_id: number; count: number };
export type GlobalFactId = 'unanimous' | 'owner' | 'newest' | 'last_winner' | 'self' | 'top_share';
export type GlobalFact = { id: GlobalFactId; pct: number };

export type GlobalView =
  | { available: false; reason: string; min_members: number }
  | {
      available: true;
      day: string;
      language: 'fr' | 'en';
      question: string;
      my_vote: number | null;
      room: { members: number; voters: number; tally: GlobalCount[] | null };
      global: { rooms: number; voters: number; min_rooms: number; facts: GlobalFact[] | null };
      yesterday: { day: string; question: string; tally: GlobalCount[]; rooms: number; facts: GlobalFact[] | null } | null;
    };

export type GlobalOpen = Extract<GlobalView, { available: true }>;

const globalKeys = {
  all: ['global'] as const,
};

export function useGlobal() {
  const { data: features } = useQuery(featuresQuery);
  const on = !!features?.includes('global_question');
  return useQuery({ queryKey: globalKeys.all, queryFn: () => api<GlobalView>('/global'), enabled: on });
}

export function useGlobalVote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (target: number) => api<GlobalView>('/global/vote', { method: 'POST', json: { target } }),
    onSuccess: (view) => qc.setQueryData(globalKeys.all, view),
    onError: () => qc.invalidateQueries({ queryKey: globalKeys.all }),
  });
}
