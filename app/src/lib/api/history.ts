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
  kind: QuestionKind;
  anonymous?: boolean;
};

export const historyExportUrl = async (tickets: boolean) =>
  `${API_URL}/api/history/export?${await urlAuth('export', tickets)}${roomQuery()}&tz=${-new Date().getTimezoneOffset()}`;

export const HISTORY_PAGE = 30;

export const getHistoryPage = (before: number | null, room?: number) =>
  api<HistoryItem[]>(before == null ? '/history' : `/history?before=${before}`, { room });

export const useHistory = () =>
  useInfiniteQuery({
    queryKey: historyKeys.list,
    queryFn: ({ pageParam }) => getHistoryPage(pageParam),
    initialPageParam: null as number | null,
    getNextPageParam: (last) => (last.length < HISTORY_PAGE ? undefined : last.at(-1)?.number),
  });

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

export const usePinned = () => useQuery({ queryKey: historyKeys.pinned, queryFn: () => api<HistoryItem[]>('/history/pinned') });
