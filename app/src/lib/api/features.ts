import { api, API_URL, ApiError } from '@/lib/api/client';
import { useQuery } from '@tanstack/react-query';
import { PROVIDERS } from '@/lib/api/auth';

export const featureKeys = {
  all: ['features'] as const,
};

export const featuresQuery = { queryKey: featureKeys.all, queryFn: () => api<string[]>('/features'), staleTime: Infinity };

export function useFeatures() {
  const known = !!API_URL;
  const query = useQuery({
    ...featuresQuery,
    retry: 2,
    enabled: known,
  });
  const { data, isError, error } = query;
  const on = new Set(data ?? []);
  return {
    loaded: !known || !!data || isError,
    unreachable: known && !data && isError && !(error instanceof ApiError && error.status === 404),
    query,
    auth: on.has('auth'),
    email: on.has('email'),
    providers: PROVIDERS.filter((p) => on.has(`oauth_${p}`)),
    consent: on.has('consent'),
    vibes: on.has('vibes'),
    languages: on.has('languages'),
    askQuestions: on.has('ask_questions'),
    notificationSettings: on.has('notification_settings'),
    avatars: on.has('avatars'),
    community: on.has('community'),
    party: on.has('party'),
    reorder: on.has('reorder'),
    choose: on.has('choose'),
    donate: on.has('donate'),
    accountRecovery: on.has('account_recovery'),
    moderation: on.has('moderation'),
    packChoose: on.has('pack_choose'),
    packTemplate: on.has('pack_template'),
    shareLinks: on.has('share_links'),
    chatReactions: on.has('chat_reactions'),
    clearQueue: on.has('clear_queue'),
    batchDelete: on.has('batch_delete'),
    chatImages: on.has('chat_images'),
    chatGifs: on.has('chat_gifs'),
    streaks: on.has('streaks'),
    anonymousVotes: on.has('anonymous_votes'),
    today: on.has('today'),
    tripot: on.has('tripot'),
    resultReactions: on.has('result_reactions'),
    said: on.has('said'),
    badges: on.has('badges'),
    wrapped: on.has('wrapped'),
    roomThemes: on.has('room_themes'),
  };
}
