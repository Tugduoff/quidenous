import type { Lang } from '@/lib/i18n-core';
import { api } from '@/lib/api/client';
import { useQuery } from '@tanstack/react-query';
import { localizeGroup } from '@/lib/utils/names';

export const groupKeys = {
  all: ['group'] as const,
  vibes: ['vibes'] as const,
};

/** `left`: someone who left or was removed (kept for names and photos in the history). */
/** `avatar`: a drawn avatar's spec (lib/avatar.ts), only when there is no photo. */
/** `deleted`: the account was deleted (the name is kept; shown as "Compte supprimé", see lib/names.ts). */
export type Member = { id: number; name: string; color: string; photo_url: string | null; avatar?: string | null; left?: boolean; deleted?: boolean };

export type AnonymousMode = 'never' | 'per_question' | 'always';

/** A count of agreements (skip someone, remove someone), and whether mine is one of them. */
export type VoteCount = { target_id: number; count: number; mine: boolean };

export type Role = 'owner' | 'member';

export type Group = {
  id: number;
  name: string;
  code: string;
  my_role: Role;
  /** A party (docs/PARTY.md) has its own screen (absent from an older server: a room). */
  kind?: 'room' | 'party';
  /** The language of the room's built-in questions (absent from an older server: French). */
  language?: 'fr' | 'en';
  /**
   * `vibe`: the built-in deck's tone (null = none). `owner_next`: only the owner launches the next question.
   * `next_delay`: seconds to wait before the next one (0 = none). All three absent from an older server.
   */
  settings: {
    skip_vote: boolean;
    kick_vote: boolean;
    questions_by: 'everyone' | 'owner';
    vibe?: string | null;
    owner_next?: boolean;
    next_delay?: number;
    /** Anonymous votes: never, each question says, or always (absent from an older server). */
    anonymous_votes?: AnonymousMode;
    /** Le Tripot on or off (rooms only; absent from an older server). */
    tripot?: boolean;
  };
  /** Pacing between questions: waiting for the owner or the delay, and when the next opens by itself (absent from an older server). */
  pace?: { waiting: boolean; next_open_at: string | null; round_id?: number | null };
  /** Current members only. */
  members: (Member & { role: Role; user_id: number })[];
  /** People who left or were removed (absent from an older server). */
  former?: (Member & { user_id?: number | null })[];
  kicks: VoteCount[];
  kick_needed: number;
};

/** A tone for a room's built-in questions; `icon` is a lucide name in kebab-case. */
export type Vibe = { id: string; name: string; icon: string; description: string };

/** Owner: open the next question now (pacing). `round_id`: the question opened, null if none could be. */
export const launchNext = (roomId: number) => api<{ round_id: number | null }>(`/rooms/${roomId}/next`, { method: 'POST' });

export const getGroup = (room?: number) => api<Group>('/group', { room });

/** The room, deleted accounts named in my language (`select` runs again when the screen re-renders on a language change). */
export const useGroup = () =>
  useQuery({ queryKey: groupKeys.all, queryFn: () => getGroup(), select: (g) => localizeGroup(g) });

/** The tones a room can pick (same for every room, so kept for the session). Only asked of a server that has them. */
export const useVibes = (enabled: boolean) =>
  useQuery({ queryKey: groupKeys.vibes, queryFn: () => api<Vibe[]>('/vibes'), staleTime: Infinity, enabled });

/**
 * id → member, for avatars everywhere: people who left too, so the history keeps their names and photos
 * (a current member wins on an id clash). `members` is only who is in the room now.
 */
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
