import type { QuestionKind } from '@/lib/api/rounds';
import { API_URL, urlAuth, roomQuery, api } from '@/lib/api/client';
import { useInfiniteQuery, type QueryClient, type InfiniteData, useQuery } from '@tanstack/react-query';

export const historyKeys = {
  all: ['history'] as const,
  list: ['history', 'list'] as const,
  pinned: ['history', 'pinned'] as const,
};

export type HistoryItem = {
  id: number;
  number: number;
  question: string;
  closed_at: string;
  winner_ids: number[];
  top_count: number;
  vote_count: number;
  pinned: boolean;
  /** Choose: `winner_ids` are the best guessers, `top_count` their hits. Said: the top scorers, `top_count` their points. */
  kind: QuestionKind;
  anonymous?: boolean;
};

/** Plain link to the CSV of the whole history (a browser downloads it). */
export const historyExportUrl = async (tickets: boolean) =>
  `${API_URL}/api/history/export?${await urlAuth('export', tickets)}${roomQuery()}&tz=${-new Date().getTimezoneOffset()}`;

/** Same size as `HISTORY_PAGE` on the server: a shorter page is the last one. */
export const HISTORY_PAGE = 30;

export const getHistoryPage = (before: number | null, room?: number) =>
  api<HistoryItem[]>(before == null ? '/history' : `/history?before=${before}`, { room });

/** Finished questions, newest first, a page at a time: `before` is the number of the oldest one shown. */
export const useHistory = () =>
  useInfiniteQuery({
    queryKey: historyKeys.list,
    queryFn: ({ pageParam }) => getHistoryPage(pageParam),
    initialPageParam: null as number | null,
    getNextPageParam: (last) => (last.length < HISTORY_PAGE ? undefined : last.at(-1)?.number),
  });

/**
 * A question just closed: fetch only the newest page and put what's new in front of the pages already loaded
 * (an invalidation would refetch every page, one after the other).
 */
export async function refreshHistory(qc: QueryClient) {
  if (!qc.getQueryData(historyKeys.list)) return;
  const fresh = await getHistoryPage(null);
  qc.setQueryData<InfiniteData<HistoryItem[], number | null>>(historyKeys.list, (old) => {
    if (!old?.pages.length) return old;
    const byId = new Map(fresh.map((i) => [i.id, i]));
    const known = new Set(old.pages.flat().map((i) => i.id));
    const pages = old.pages.map((page) => page.map((i) => byId.get(i.id) ?? i));
    pages[0] = [...fresh.filter((i) => !known.has(i.id)), ...pages[0]];
    return { ...old, pages };
  });
}

/** Results pinned for the whole group (all of them, not paged). */
export const usePinned = () => useQuery({ queryKey: historyKeys.pinned, queryFn: () => api<HistoryItem[]>('/history/pinned') });
