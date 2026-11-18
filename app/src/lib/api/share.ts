import { api, appUrl } from '@/lib/api/client';
import { queryClient } from '@/lib/api/query-client';
import { useQuery } from '@tanstack/react-query';

/** One of my public links (`GET /shares`): a question (with its result or not) or a stats story. */
export type MyLink = {
  token: string;
  path: string;
  kind: 'question' | 'stat';
  title: string;
  room: string;
  with_result: boolean;
  created_at: string;
  /** Null for links made before links expired. */
  expires_at: string | null;
};

export const shareKeys = {
  mine: ['shares'] as const,
};

const myLinks = () => api<MyLink[]>('/shares');

export const useMyLinks = (enabled: boolean) => useQuery({ queryKey: shareKeys.mine, queryFn: myLinks, enabled });

export const shareRound = (roundId: number, result: boolean) =>
  api<{ token?: string; path: string }>(`/rounds/${roundId}/share`, { method: 'POST', json: { result } });

export const revokeLink = (token: string) => api(`/shares/${token}`, { method: 'DELETE' });

/** Public link to a card of this question (with its podium when `result`), previewed as an image in chats. */
export async function publicLink(roundId: number, result: boolean): Promise<string | null> {
  try {
    const r = await shareRound(roundId, result);
    queryClient.invalidateQueries({ queryKey: shareKeys.mine });
    return `${appUrl()}${r.path}`;
  } catch {
    return null;
  }
}
