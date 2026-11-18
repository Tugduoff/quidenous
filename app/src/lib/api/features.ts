import { api, API_URL, ApiError } from '@/lib/api/client';
import { useQuery } from '@tanstack/react-query';
import { PROVIDERS } from '@/lib/api/auth';

/** `/api/features`, server-wide: kept when a room switch or a sign-in clears the cache (lib/query-client.ts). */
export const featureKeys = {
  all: ['features'] as const,
};

export const featuresQuery = { queryKey: featureKeys.all, queryFn: () => api<string[]>('/features'), staleTime: Infinity };

/**
 * What the server can do (`/api/features`). An older server has no "auth": the app keeps the old join screen.
 * `query` failed for another reason than "no such route" (offline, server down): `unreachable`, never "old server".
 */
export function useFeatures() {
  // No server known yet (phone build without a baked-in URL): the old screen, which asks for one.
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
    /** The server couldn't be reached (an older server answers 404 instead): show a retry, not the old screen. */
    unreachable: known && !data && isError && !(error instanceof ApiError && error.status === 404),
    query,
    auth: on.has('auth'),
    email: on.has('email'),
    providers: PROVIDERS.filter((p) => on.has(`oauth_${p}`)),
    /** Friend requests to accept and room invitations (`/invites`) instead of instant adds. */
    consent: on.has('consent'),
    /** Rooms can pick a tone for their built-in questions (`/vibes`). */
    vibes: on.has('vibes'),
    /** The app in French or English, and each room picks the language of its questions (`language`). */
    languages: on.has('languages'),
    /** A member who can't add questions can ask the owner for some (`/questions/ask`). */
    askQuestions: on.has('ask_questions'),
    /** Per-account push settings (`/notifications`) and muting a room (`/rooms/{id}/mute`). */
    notificationSettings: on.has('notification_settings'),
    /** Drawn avatars (`PUT /me/avatar`). */
    avatars: on.has('avatars'),
    /** Packs 2.0, suggestions to the deck, share → play, challenges (docs/COMMUNITY.md). */
    community: on.has('community'),
    /** Party mode: one question at a time for people in the same place (docs/PARTY.md). */
    party: on.has('party'),
    /** The room's upcoming questions can be put in any order (`POST /questions/order`). */
    reorder: on.has('reorder'),
    /** Choose questions (wire people to items). */
    choose: on.has('choose'),
    /** A "buy us a coffee" page (`/donate`, docs/DONATE.md); shown only on the web and the sideloaded APK. */
    donate: on.has('donate'),
    /** Change e-mail, 2FA reset by e-mail, `email_failed` (docs/AUTH.md). */
    accountRecovery: on.has('account_recovery'),
    /** Deleting chat messages, removing asked questions, reports an admin reviews, locking a party. */
    moderation: on.has('moderation'),
    /** Packs carry choose questions (with their items). */
    packChoose: on.has('pack_choose'),
    /** The pack file template for AI agents (`/pack-template.md`, `/en/pack-template.md`). */
    packTemplate: on.has('pack_template'),
    /** My public links can be listed and deleted (`/shares`). */
    shareLinks: on.has('share_links'),
    /** Emoji reactions on chat messages (`POST/DELETE /messages/{id}/reactions`). */
    chatReactions: on.has('chat_reactions'),
    /** The owner can empty the room's upcoming questions at once (`DELETE /questions/queue`). */
    clearQueue: on.has('clear_queue'),
    /** Several questions or packs deleted in one request (`POST /questions/delete`, `POST /packs/delete`). */
    batchDelete: on.has('batch_delete'),
    /** Photos in the result chat (`POST /rounds/{id}/messages/image`). */
    chatImages: on.has('chat_images'),
    /** GIFs in the result chat, searched through the server (`/gifs`, KLIPY). */
    chatGifs: on.has('chat_gifs'),
    /** Days in a row with a vote (`/streaks`, `day_streaks` in /stats, the "streak" push; docs/STREAKS.md). */
    streaks: on.has('streaks'),
    /** Anonymous votes: the room rule, per-question flag, counts-only results (docs/ANONYMOUS.md). */
    anonymousVotes: on.has('anonymous_votes'),
    /** « Aujourd'hui »: every room on one screen (`GET /today`) and other rooms' activity on the live socket (`others=1`). */
    today: on.has('today'),
    /** Le Tripot: bets with chips worth nothing (`/rounds/{id}/bet`, `/tripot`; docs/TRIPOT.md). */
    tripot: on.has('tripot'),
    /** Live emoji reactions on results, taps with bursts (`/rounds/{id}/reactions`, `result_reacted`; docs/REACTIONS.md). */
    resultReactions: on.has('result_reactions'),
    /** « Qui a dit ça ? » questions: write, guess who wrote what (docs/SAID.md). */
    said: on.has('said'),
    /** Badges earned from play and one shown as a title (`/badges`, `/titles`, `PUT /me/title`). */
    badges: on.has('badges'),
    /** « Ton année qui de nous »: story cards of the year (`/wrapped`, `POST /wrapped/share`). */
    wrapped: on.has('wrapped'),
  };
}
