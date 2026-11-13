import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { api, ApiError, API_URL } from './api';
import { authError, useFeatures } from './auth';
import { getLang, t, type TKey } from './i18n';

/** Suggestions to the official deck, share → play and challenges (docs/COMMUNITY.md). */

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
/** 1 = up, -1 = down, 0 = skipped. */
export type SuggestionVote = 1 | -1 | 0;

/** `GET /play/{token}` (public): the shared question; `closed`: its round is over, so playing it makes a challenge. */
export type PlayInfo = { question: string; kind: 'pick' | 'rank'; lang: 'fr' | 'en'; room: string; closed: boolean };

/** Someone on a podium of another room (or of this one): no member id, only what the avatar needs. */
export type PodiumEntry = { name: string; color: string; avatar?: string | null; photo_url: string | null; score: number };
/** A room this round was sent to or came from; podiums only once both rounds are closed. */
export type Challenge = { room: string; round_id: number | null; closed: boolean; podium: PodiumEntry[] | null; mine_podium: PodiumEntry[] | null };

/** How many suggestions a day (the server's rolling 24 h limit). */
export const SUGGESTIONS_PER_DAY = 5;
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

/** A short message for a failed community call, in the current language. */
export function communityError(e: unknown): string {
  if (e instanceof ApiError && ERRORS[e.code]) return t(ERRORS[e.code]);
  return authError(e);
}

/** My suggestions (newest first) and how many I can still send today. */
export const useMySuggestions = (enabled = true) =>
  useQuery({
    queryKey: communityKeys.mine,
    queryFn: () => api<{ left: number; items: Suggestion[] }>('/suggestions/mine'),
    enabled,
  });

/** Others' suggestions in my language that I have not voted on yet (up to 20, oldest first). */
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
    mutationFn: (body: { text: string; kind: 'pick' | 'rank' }) =>
      api<Suggestion>('/suggestions', { method: 'POST', json: { ...body, lang: getLang() } }),
    onSettled: () => qc.invalidateQueries({ queryKey: communityKeys.mine }),
  });
}

export const VOTE_KEY = ['suggestion-vote'] as const;

export function useVoteSuggestion() {
  return useMutation({
    mutationKey: VOTE_KEY,
    mutationFn: ({ id, value }: { id: number; value: SuggestionVote }) =>
      api<{ up: number; down: number; status: SuggestionStatus }>(`/suggestions/${id}/vote`, { method: 'POST', json: { value } }),
  });
}

/** The shared question behind a `/play?share=` link (no account needed). */
export const usePlay = (token: string | null) =>
  useQuery({
    queryKey: communityKeys.play(token ?? ''),
    queryFn: () => api<PlayInfo>(`/play/${encodeURIComponent(token!)}`),
    enabled: !!token,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
    staleTime: Infinity,
  });

/** Add the shared question to one of my rooms (`X-Room` = that room), as its next question. */
export const playInto = (token: string, roomId: number) =>
  api<{ question_id: number; challenge_id: number | null }>(`/play/${encodeURIComponent(token)}`, { method: 'POST', room: roomId });

/** Every room this (closed) round was sent to or came from. Only asked of a server that has them. */
export function useChallenges(roundId: number, enabled = true) {
  const { community } = useFeatures();
  return useQuery({
    queryKey: communityKeys.challenges(roundId),
    queryFn: () => api<Challenge[]>(`/rounds/${roundId}/challenges`),
    enabled: enabled && community,
  });
}

/** Send a closed round to another of my rooms (from the current one). */
export const challengeRoom = (roundId: number, roomId: number) =>
  api<{ challenge_id: number }>(`/rounds/${roundId}/challenge`, { method: 'POST', json: { room_id: roomId } });

/** A public link to this question (no podium) whose page leads to /play. */
export async function questionShareLink(roundId: number): Promise<string> {
  const r = await api<{ token?: string; path: string }>(`/rounds/${roundId}/share`, { method: 'POST', json: { result: false } });
  const base = API_URL || (typeof location !== 'undefined' ? location.origin : '');
  return `${base}${r.token ? `/s/${r.token}` : r.path}`;
}
