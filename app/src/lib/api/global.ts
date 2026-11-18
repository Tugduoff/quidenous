import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { api } from '@/lib/api/client';
import { featuresQuery } from '@/lib/api/auth';

/** The global question of the day (docs/GLOBAL_QUESTION.md). */
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
      /** My room: its size, how many answered, the counts once I voted (never who voted). */
      room: { members: number; voters: number; tally: GlobalCount[] | null };
      /** Rooms of this language with 2+ votes; the facts once I voted and at least `min_rooms` rooms answered. */
      global: { rooms: number; voters: number; min_rooms: number; facts: GlobalFact[] | null };
      yesterday: { day: string; question: string; tally: GlobalCount[]; rooms: number; facts: GlobalFact[] | null } | null;
    };

export type GlobalOpen = Extract<GlobalView, { available: true }>;

export const globalKey = ['global'] as const;

/** Today's global question in the current room, only asked of a server that has it. */
export function useGlobal() {
  const { data: features } = useQuery(featuresQuery);
  const on = !!features?.includes('global_question');
  return useQuery({ queryKey: globalKey, queryFn: () => api<GlobalView>('/global'), enabled: on });
}

/** Votes (final): the answer is the new view (my room's tally, the aggregates). */
export function useGlobalVote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (target: number) => api<GlobalView>('/global/vote', { method: 'POST', json: { target } }),
    onSuccess: (view) => qc.setQueryData(globalKey, view),
    // Voted already (another phone), someone left: show what is true now.
    onError: () => qc.invalidateQueries({ queryKey: globalKey }),
  });
}
