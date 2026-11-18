import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

export const friendKeys = {
  all: ['friends'] as const,
};



export type Friend = {
  user_id: number;
  name: string;
  color: string;
  photo_url: string | null;
  avatar?: string | null;
  rooms_in_common: number;
  added: boolean;
  /** I asked them to be friends (a room-mate kept for good), waiting for them (absent from an older server). */
  requested?: boolean;
};

/** Someone in a friend request (`incoming` / `outgoing` of `GET /friends`). */
export type Person = Pick<Friend, 'user_id' | 'name' | 'color' | 'photo_url' | 'avatar'>;

/** `GET /friends`; `incoming` / `outgoing` friend requests are absent from an older server. */
export type Friends = { code: string; friends: Friend[]; incoming?: Person[]; outgoing?: Person[] };

export const useFriends = (enabled = true) => useQuery({ queryKey: friendKeys.all, queryFn: () => api<Friends>('/friends'), enabled });
