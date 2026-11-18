import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import type { Role } from '@/lib/api/group';
import type { QuestionKind } from '@/lib/api/rounds';
import { useFeatures } from '@/lib/api/features';

export const todayKeys = {
  all: ['today'] as const,
};

type TodayState = 'vote' | 'result' | 'tripot' | 'wait' | 'idle';

export type TodayRoom = {
  id: number;
  name: string;
  role: Role;
  members: number;
  muted: boolean;
  state: TodayState;
  round: { id: number; number: number; text: string; kind: QuestionKind; anonymous: boolean; opened_at: string } | null;
  voted: boolean;
  votes: number;
  needed: number;
  result: { id: number; number: number; text: string; kind: QuestionKind; closed_at: string } | null;
  unseen: number;
  tripot: { can_bet: boolean; balance: number } | null;
  streak: { days: number; today: boolean } | null;
  unread: number;
  next_open_at: string | null;
};

export type Today = { rooms: TodayRoom[] };

export const useToday = (active: boolean) => {
  const { today } = useFeatures();
  return useQuery({ queryKey: todayKeys.all, queryFn: () => api<Today>('/today'), enabled: today, subscribed: active, staleTime: 0 });
};
