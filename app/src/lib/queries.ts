import { keepPreviousData, type InfiniteData, type QueryClient, useInfiniteQuery, useQuery } from '@tanstack/react-query';

import { api, type Friends, type Group, type RoomSummary, type HistoryItem, type InboxItem, type Message, type Member, type Queue, type RoomInvite, type Round, normalizeStats, type Stats, type StatsPeriod, type Vibe } from './api';
import { localizeGroup } from './names';

export const keys = {
  group: ['group'] as const,
  current: ['round', 'current'] as const,
  unseen: ['round', 'unseen'] as const,
  round: (id: number) => ['round', id] as const,
  history: ['history'] as const,
  /** The paged list alone (`history` also covers the pinned ones). */
  historyList: ['history', 'list'] as const,
  pinned: ['history', 'pinned'] as const,
  /** Every period's stats (what a live event invalidates). */
  stats: ['stats'] as const,
  statsFor: (period: StatsPeriod) => ['stats', period] as const,
  messages: (id: number) => ['messages', id] as const,
  inbox: ['inbox'] as const,
  rooms: ['rooms'] as const,
  friends: ['friends'] as const,
  /** Room invitations for me. */
  invites: ['invites'] as const,
  /** Who has a pending invitation into a room. */
  roomInvites: (id: number) => ['invites', 'room', id] as const,
  queue: ['queue'] as const,
  blocks: ['blocks'] as const,
  vibes: ['vibes'] as const,
};

/** The room, deleted accounts named in my language (`select` runs again when the screen re-renders on a language change). */
export const useGroup = () =>
  useQuery({ queryKey: keys.group, queryFn: () => api<Group>('/group'), select: (g) => localizeGroup(g) });

export const useCurrentRound = () =>
  useQuery({ queryKey: keys.current, queryFn: () => api<Round | null>('/round/current') });

/**
 * A finished question I voted on but have not seen the result of (I was away when it closed): the oldest, with how many
 * are waiting and the newest of them (marking that one seen skips them all). Both absent from an older server.
 */
export type UnseenRound = Round & { unseen_count?: number; unseen_last_id?: number };
export const useUnseenRound = () =>
  useQuery({ queryKey: keys.unseen, queryFn: () => api<UnseenRound | null>('/round/unseen') });

export const useRound = (id: number | null) =>
  useQuery({ queryKey: keys.round(id ?? 0), queryFn: () => api<Round>(`/rounds/${id}`), enabled: id != null });

/** Same size as `HISTORY_PAGE` on the server: a shorter page is the last one. */
const HISTORY_PAGE = 30;

/** Finished questions, newest first, a page at a time: `before` is the number of the oldest one shown. */
export const useHistory = () =>
  useInfiniteQuery({
    queryKey: keys.historyList,
    queryFn: ({ pageParam }) => api<HistoryItem[]>(pageParam == null ? '/history' : `/history?before=${pageParam}`),
    initialPageParam: null as number | null,
    getNextPageParam: (last) => (last.length < HISTORY_PAGE ? undefined : last.at(-1)!.number),
  });

/**
 * A question just closed: fetch only the newest page and put what's new in front of the pages already loaded
 * (an invalidation would refetch every page, one after the other).
 */
export async function refreshHistory(qc: QueryClient) {
  if (!qc.getQueryData(keys.historyList)) return;
  const fresh = await api<HistoryItem[]>('/history');
  qc.setQueryData<InfiniteData<HistoryItem[], number | null>>(keys.historyList, (old) => {
    if (!old?.pages.length) return old;
    const byId = new Map(fresh.map((i) => [i.id, i]));
    const known = new Set(old.pages.flat().map((i) => i.id));
    const pages = old.pages.map((page) => page.map((i) => byId.get(i.id) ?? i));
    pages[0] = [...fresh.filter((i) => !known.has(i.id)), ...pages[0]];
    return { ...old, pages };
  });
}

/** Results pinned for the whole group (all of them, not paged). */
export const usePinned = () => useQuery({ queryKey: keys.pinned, queryFn: () => api<HistoryItem[]>('/history/pinned') });

export const useMessages = (roundId: number) =>
  useQuery({ queryKey: keys.messages(roundId), queryFn: () => api<Message[]>(`/rounds/${roundId}/messages`) });

export const useInbox = () => useQuery({ queryKey: keys.inbox, queryFn: () => api<{ items: InboxItem[]; unread: number }>('/inbox') });

export const useRooms = () => useQuery({ queryKey: keys.rooms, queryFn: () => api<RoomSummary[]>('/rooms') });

export const useFriends = (enabled = true) => useQuery({ queryKey: keys.friends, queryFn: () => api<Friends>('/friends'), enabled });

/** Room invitations waiting for me (a server without them: none). */
export const useInvites = (enabled = true) =>
  useQuery({ queryKey: keys.invites, queryFn: async () => (await api<{ invites: RoomInvite[] }>('/invites')).invites, enabled });

/** User ids with a pending invitation into this room. */
export const useRoomInvites = (room: number | undefined, enabled = true) =>
  useQuery({
    queryKey: keys.roomInvites(room ?? 0),
    queryFn: async () => (await api<{ user_ids: number[] }>(`/rooms/${room}/invites`)).user_ids,
    enabled: enabled && room != null,
  });

/** The tones a room can pick (same for every room, so kept for the session). Only asked of a server that has them. */
export const useVibes = (enabled: boolean) =>
  useQuery({ queryKey: keys.vibes, queryFn: () => api<Vibe[]>('/vibes'), staleTime: Infinity, enabled });

/** Stats of a period, in the phone's time zone (`enabled`: only once needed). Switching period keeps the previous numbers on screen until the new ones land. */
export const useStats = (period: StatsPeriod, enabled = true) =>
  useQuery({
    queryKey: keys.statsFor(period),
    enabled,
    queryFn: async () => normalizeStats(await api<Partial<Stats> | null>(`/stats?period=${period}&tz=${-new Date().getTimezoneOffset()}`), period),
    placeholderData: keepPreviousData,
  });

export const useQueue = () => useQuery({ queryKey: keys.queue, queryFn: () => api<Queue>('/questions/queue') });

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
