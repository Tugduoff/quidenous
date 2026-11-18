import type { Person } from '@/lib/api/friends';
import type { Role } from '@/lib/api/group';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

export const roomKeys = {
  all: ['rooms'] as const,
  invites: ['invites'] as const,
  roomInvites: (roomId: number) => ['invites', 'room', roomId] as const,
  preview: (code: string) => ['room-preview', code] as const,
};

/** A room someone invited me into (`GET /invites`). */
export type RoomInvite = { id: number; room_id: number; room_name: string; members: number; created_at: string; from: Person };

/** `muted`: no pushes from this room for me (absent from an older server). `kind`, `ends_at`: a party and when it ends (docs/PARTY.md). */
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
  /** A party past its end: a read-only memory. */
  over?: boolean;
};

export const useRooms = () => useQuery({ queryKey: roomKeys.all, queryFn: () => api<RoomSummary[]>('/rooms') });

/** Room invitations waiting for me (a server without them: none). */
export const useInvites = (enabled = true) =>
  useQuery({ queryKey: roomKeys.invites, queryFn: async () => (await api<{ invites: RoomInvite[] }>('/invites')).invites, enabled });

/** User ids with a pending invitation into this room. */
export const useRoomInvites = (room: number | undefined, enabled = true) =>
  useQuery({
    queryKey: roomKeys.roomInvites(room ?? 0),
    queryFn: async () => (await api<{ user_ids: number[] }>(`/rooms/${room}/invites`)).user_ids,
    enabled: enabled && room != null,
  });
