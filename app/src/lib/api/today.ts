// « Aujourd'hui »: every room of mine on one screen, what each one wants from me today (GET /api/today).
// Also when the app opens on it: with two rooms or more, at a plain cold start (see `useLandOnToday`).
import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import type { Role } from '@/lib/api/group';
import type { QuestionKind } from '@/lib/api/rounds';
import { useFeatures } from '@/lib/api/features';

export const todayKeys = {
  all: ['today'] as const,
};

/** What a room wants from me, most pressing first (the server orders the rooms by it). */
export type TodayState = 'vote' | 'result' | 'tripot' | 'wait' | 'idle';

export type TodayRoom = {
  id: number;
  name: string;
  role: Role;
  members: number;
  muted: boolean;
  state: TodayState;
  /** The open question, null when none. */
  round: { id: number; number: number; text: string; kind: QuestionKind; anonymous: boolean; opened_at: string } | null;
  voted: boolean;
  /** Votes in and votes needed to close (people skipped or away aren't waited for). */
  votes: number;
  needed: number;
  /** The newest result I voted on and haven't seen, `unseen` of them in all. */
  result: { id: number; number: number; text: string; kind: QuestionKind; closed_at: string } | null;
  unseen: number;
  /** Null when the room has no Tripot; `can_bet`: I can still bet on the open question. */
  tripot: { can_bet: boolean; balance: number } | null;
  /** My day streak in this room, and whether today already counts. */
  streak: { days: number; today: boolean } | null;
  unread: number;
  /** A paused room: when its next question opens by itself (null: the owner launches it). */
  next_open_at: string | null;
};

export type Today = { rooms: TodayRoom[] };

/**
 * Every room at once. `active`: only the focused tab listens, so events in other rooms don't refetch it behind
 * the vote screen; back on the tab it refetches if anything changed meanwhile (stale at once: each visit is fresh).
 */
export const useToday = (active: boolean) => {
  const { today } = useFeatures();
  return useQuery({ queryKey: todayKeys.all, queryFn: () => api<Today>('/today'), enabled: today, subscribed: active, staleTime: 0 });
};
