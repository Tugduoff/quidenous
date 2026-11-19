import type { Lang } from '@/lib/i18n-core';
import { api } from '@/lib/api/client';
import { useQuery } from '@tanstack/react-query';
import { localizeGroup } from '@/lib/utils/names';

export const groupKeys = {
  all: ['group'] as const,
  vibes: ['vibes'] as const,
};

export type Member = { id: number; name: string; color: string; photo_url: string | null; avatar?: string | null; left?: boolean; deleted?: boolean };

export type AnonymousMode = 'never' | 'per_question' | 'always';

export type VoteCount = { target_id: number; count: number; mine: boolean };

export type Role = 'owner' | 'member';

export type Group = {
  id: number;
  name: string;
  code: string;
  my_role: Role;
  kind?: 'room' | 'party';
  language?: 'fr' | 'en';
  settings: {
    skip_vote: boolean;
    kick_vote: boolean;
    questions_by: 'everyone' | 'owner';
    vibe?: string | null;
    owner_next?: boolean;
    next_delay?: number;
    anonymous_votes?: AnonymousMode;
    tripot?: boolean;
  };
  pace?: { waiting: boolean; next_open_at: string | null; round_id?: number | null };
  members: (Member & { role: Role; user_id: number })[];
  former?: (Member & { user_id?: number | null })[];
  kicks: VoteCount[];
  kick_needed: number;
};

export type Vibe = { id: string; name: string; icon: string; description: string };

export const launchNext = (roomId: number) => api<{ round_id: number | null }>(`/rooms/${roomId}/next`, { method: 'POST' });

export const getGroup = (room?: number) => api<Group>('/group', { room });

export const useGroup = () =>
  useQuery({ queryKey: groupKeys.all, queryFn: () => getGroup(), select: (g) => localizeGroup(g) });

export const useVibes = (enabled: boolean) =>
  useQuery({ queryKey: groupKeys.vibes, queryFn: () => api<Vibe[]>('/vibes'), staleTime: Infinity, enabled });

export function useMembers() {
  const { data } = useGroup();
  const everyone: (Member & { user_id?: number | null })[] = [...(data?.former ?? []), ...(data?.members ?? [])];
  const byId = new Map(everyone.map((m) => [m.id, m]));
  return { members: data?.members ?? [], byId };
}

export type RoomPatch = Partial<Group['settings']> & { name?: string; language?: Lang };

export const updateRoom = (roomId: number, patch: RoomPatch) => api(`/rooms/${roomId}`, { method: 'PATCH', json: patch });

export const renewRoomCode = (roomId: number) => api(`/rooms/${roomId}/code`, { method: 'POST' });

export const transferOwnership = (roomId: number, memberId: number) => api(`/rooms/${roomId}/owner`, { method: 'POST', json: { member_id: memberId } });

export const deleteRoom = (roomId: number) => api(`/rooms/${roomId}`, { method: 'DELETE' });

export const leaveRoom = (roomId: number) => api(`/rooms/${roomId}/leave`, { method: 'POST' });

export const removeMember = (roomId: number, memberId: number) => api(`/rooms/${roomId}/members/${memberId}`, { method: 'DELETE' });

export const voteKick = (memberId: number, on: boolean) => api(`/members/${memberId}/kick`, { method: on ? 'PUT' : 'DELETE' });
