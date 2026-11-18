import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

export const inboxKeys = {
  all: ['inbox'] as const,
};



export type InboxItem = {
  id: number;
  /** Unknown kinds (a newer server) are shown with a generic icon and title. */
  kind: 'message' | 'mention' | 'nudge' | 'result' | 'question' | 'added' | 'ask' | 'badge' | 'wrapped' | (string & {});
  round_id: number | null;
  /** The room it happened in (absent from an older server). */
  room_id?: number;
  actor_id: number | null;
  text: string;
  created_at: string;
  read: boolean;
};

export const useInbox = () => useQuery({ queryKey: inboxKeys.all, queryFn: () => api<{ items: InboxItem[]; unread: number }>('/inbox') });

export const markInboxRead = () => api('/inbox/read', { method: 'POST' });
