import type { Person } from '@/lib/api/friends';
import { type Role, groupKeys, getGroup } from '@/lib/api/group';
import { useQuery, type QueryKey } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { roundKeys, getCurrentRound, getUnseenRound, getRound } from '@/lib/api/rounds';

export const roomKeys = {
  all: ['rooms'] as const,
  invites: ['invites'] as const,
  roomInvites: (roomId: number) => ['invites', 'room', roomId] as const,
  preview: (code: string) => ['room-preview', code] as const,
};

export type RoomInvite = { id: number; room_id: number; room_name: string; members: number; created_at: string; from: Person };

export type RoomSummary = {
  id: number;
  name: string;
  role: Role;
  members: number;
  open_number: number | null;
  voted: boolean;
  unread: number;
  muted?: boolean;
  language?: 'fr' | 'en';
  kind?: 'room' | 'party';
  ends_at?: string | null;
  over?: boolean;
};

export const listRooms = () => api<RoomSummary[]>('/rooms');

export const useRooms = (enabled = true) => useQuery({ queryKey: roomKeys.all, queryFn: listRooms, enabled });

export const useInvites = (enabled = true) =>
  useQuery({ queryKey: roomKeys.invites, queryFn: async () => (await api<{ invites: RoomInvite[] }>('/invites')).invites, enabled });

export const useRoomInvites = (room: number | undefined, enabled = true) =>
  useQuery({
    queryKey: roomKeys.roomInvites(room ?? 0),
    queryFn: async () => (await api<{ user_ids: number[] }>(`/rooms/${room}/invites`)).user_ids,
    enabled: enabled && room != null,
  });

export type NewRoom = { name: string; vibe?: string; language?: 'fr' | 'en' };

export const createRoom = (room: NewRoom) => api<{ id: number }>('/rooms', { method: 'POST', json: room });

export const joinRoom = (code: string) => api<{ id: number }>('/rooms/join', { method: 'POST', json: { code } });

export const setRoomMuted = (roomId: number, mute: boolean) =>
  api<{ ok: boolean; muted: boolean }>(`/rooms/${roomId}/mute`, { method: mute ? 'PUT' : 'DELETE' });

export type RoomPreview = { name: string; member_count: number; names: string[] };

export const useRoomPreview = (code: string, enabled: boolean) =>
  useQuery({
    queryKey: roomKeys.preview(code),
    queryFn: () => api<RoomPreview>(`/rooms/preview?code=${encodeURIComponent(code)}`),
    enabled,
    retry: false,
  });

export const addToRoom = (roomId: number, userId: number) => api(`/rooms/${roomId}/add`, { method: 'POST', json: { user_id: userId } });

export const cancelRoomInvite = (roomId: number, userId: number) => api(`/rooms/${roomId}/invites/${userId}`, { method: 'DELETE' });

export const acceptInvite = (inviteId: number) => api<{ room_id: number }>(`/invites/${inviteId}/accept`, { method: 'POST' });

export const declineInvite = (inviteId: number) => api(`/invites/${inviteId}`, { method: 'DELETE' });

const WARM_MS = 500;

export async function warmRoom(id: number) {
  const got: [QueryKey, unknown][] = [];
  const get = <T>(key: QueryKey, load: () => Promise<T>) =>
    load().then(
      (data) => {
        got.push([key, data]);
        return data;
      },
      () => undefined,
    );
  const all = Promise.all([
    get(groupKeys.all, () => getGroup(id)),
    get(roundKeys.current, () => getCurrentRound(id)),
    get(roundKeys.unseen, () => getUnseenRound(id)).then((u) => u && get(roundKeys.detail(u.id), () => getRound(u.id, id))),
  ]);
  await Promise.race([all, new Promise((done) => setTimeout(done, WARM_MS))]);
  return [...got];
}
