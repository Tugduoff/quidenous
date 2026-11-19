import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError, appUrl } from '@/lib/api/client';
import { getLang, t, type TKey } from '@/lib/i18n';
import { authError } from '@/lib/api/errors';
import { useFeatures } from '@/lib/api/features';
import { shareRound } from '@/lib/api/share';

export type SuggestionStatus = 'voting' | 'queued' | 'approved' | 'rejected';
export type Suggestion = {
  id: number;
  text: string;
  kind: 'pick' | 'rank';
  lang: 'fr' | 'en';
  status: SuggestionStatus;
  up: number;
  down: number;
  author: string;
  mine: boolean;
};
export type SuggestionVote = 1 | -1 | 0;

export type PlayInfo = { question: string; kind: 'pick' | 'rank' | 'choose'; lang: 'fr' | 'en'; room: string; closed: boolean };

export type PodiumEntry = { name: string; color: string; avatar?: string | null; photo_url: string | null; score: number };
export type Challenge = {
  room: string;
  round_id: number | null;
  closed: boolean;
  podium: PodiumEntry[] | null;
  mine_podium: PodiumEntry[] | null;
  private?: boolean;
};

export const SUGGESTION_MIN = 8;
export const SUGGESTION_MAX = 200;

export const communityKeys = {
  mine: ['suggestions', 'mine'] as const,
  next: (lang: string) => ['suggestions', 'next', lang] as const,
  play: (token: string) => ['play', token] as const,
  challenges: (roundId: number) => ['challenges', roundId] as const,
};

const ERRORS: Record<string, TKey> = {
  too_many_suggestions: 'community.errors.tooMany',
  duplicate: 'community.errors.duplicate',
  own_suggestion: 'community.errors.own',
  closed: 'community.errors.closed',
  already_asked: 'community.errors.alreadyAsked',
  owner_only: 'community.errors.ownerOnly',
  not_in_room: 'community.errors.notInRoom',
  same_room: 'community.errors.sameRoom',
  not_found: 'community.errors.notFound',
};

export function communityError(e: unknown): string {
  if (e instanceof ApiError && ERRORS[e.code]) return t(ERRORS[e.code]);
  return authError(e);
}

export const useMySuggestions = (enabled = true) =>
  useQuery({
    queryKey: communityKeys.mine,
    queryFn: () => api<{ left: number; items: Suggestion[] }>('/suggestions/mine'),
    enabled,
  });

export function useNextSuggestions(enabled = true) {
  const lang = getLang();
  return useQuery({
    queryKey: communityKeys.next(lang),
    queryFn: () => api<Suggestion[]>(`/suggestions/next?lang=${lang}`),
    enabled,
    staleTime: Infinity,
  });
}

export function useSuggest() {
  const qc = useQueryClient();
  return useMutation({
    meta: { quiet: true },
    mutationFn: (body: { text: string; kind: 'pick' | 'rank' }) =>
      api<Suggestion>('/suggestions', { method: 'POST', json: { ...body, lang: getLang() } }),
    onSettled: () => qc.invalidateQueries({ queryKey: communityKeys.mine }),
  });
}

export const VOTE_KEY = ['suggestion-vote'] as const;

export function useVoteSuggestion() {
  return useMutation({
    meta: { quiet: true },
    mutationKey: VOTE_KEY,
    mutationFn: ({ id, value }: { id: number; value: SuggestionVote }) =>
      api<{ up: number; down: number; status: SuggestionStatus }>(`/suggestions/${id}/vote`, { method: 'POST', json: { value } }),
  });
}

export const usePlay = (token: string | null) =>
  useQuery({
    queryKey: communityKeys.play(token ?? ''),
    queryFn: () => (token ? api<PlayInfo>(`/play/${encodeURIComponent(token)}`) : Promise.reject(new Error('no share token'))),
    enabled: !!token,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
    staleTime: Infinity,
  });

export const playInto = (token: string, roomId: number) =>
  api<{ question_id: number; challenge_id: number | null }>(`/play/${encodeURIComponent(token)}`, { method: 'POST', room: roomId });

export function useChallenges(roundId: number, enabled = true) {
  const { community } = useFeatures();
  return useQuery({
    queryKey: communityKeys.challenges(roundId),
    queryFn: () => api<Challenge[]>(`/rounds/${roundId}/challenges`),
    enabled: enabled && community,
  });
}

export const challengeRoom = (roundId: number, roomId: number) =>
  api<{ challenge_id: number }>(`/rounds/${roundId}/challenge`, { method: 'POST', json: { room_id: roomId } });

export async function questionShareLink(roundId: number): Promise<string> {
  const r = await shareRound(roundId, false);
  const base = appUrl();
  return `${base}${r.token ? `/s/${r.token}` : r.path}`;
}
