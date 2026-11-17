import { useQuery } from '@tanstack/react-query';
import { Platform } from 'react-native';

import { errors } from '@/locales/fr/errors';

import { API_URL, api, ApiError, presetJoinCode } from './api';
import { deviceHeader } from './device';
import { getLang, t } from './i18n';

export type Provider = 'google' | 'discord';
export const PROVIDERS: Provider[] = ['google', 'discord'];
export const PROVIDER_NAMES: Record<Provider, string> = { google: 'Google', discord: 'Discord' };

/** A new session (`email_sent` / `email_failed` only on sign-up: `email_failed` = Mailgun refused it or is down). */
export type SessionResult = { token: string; user_id: number; new: boolean; email_sent?: boolean; email_failed?: boolean };
/** 2FA is on: trade the challenge and a code for the session. */
export type Challenge = { mfa: 'totp'; challenge: string };
export type AuthResult = SessionResult | Challenge;
export type RedeemResult = AuthResult | { linked: Provider } | { reauth: true };

export const isChallenge = (r: RedeemResult): r is Challenge => 'challenge' in r;
export const isLinked = (r: RedeemResult): r is { linked: Provider } => 'linked' in r;
/** A Google / Discord round that only proved it is me (before a sensitive change). */
export const isReauth = (r: RedeemResult): r is { reauth: true } => 'reauth' in r;

/** `link`: add the provider to my account; `reauth`: prove it is me (signed in, already linked). */
export type OAuthMode = 'signin' | 'link' | 'reauth';

export type Account = {
  user_id: number;
  name: string;
  email: string | null;
  email_verified: boolean;
  has_password: boolean;
  totp: boolean;
  recovery_codes_left: number;
  providers: Provider[];
  secured: boolean;
  /** A party guest: plays in its party only (no friends, rooms, packs…) until secured. Absent from an older server. */
  guest?: boolean;
  /** A 2FA reset by e-mail is pending: 2FA goes at this time (unix seconds) unless cancelled. */
  mfa_reset_at?: number | null;
  /** Accepted the current terms (16+ included); `false`: the app asks once. Absent from an older server. */
  terms?: boolean;
};

export type DeviceSession = { id: string; device: string | null; created_at: string; last_used_at: string | null; current: boolean };

/** POST /api/auth/…; `session` = the call hands out a token, so it says which device this is. */
const post = <T>(path: string, json: object = {}, session = false) =>
  api<T>(`/auth${path}`, { method: 'POST', json, headers: session ? deviceHeader() : undefined });

export const auth = {
  email: (email: string) => post<{ account: boolean }>('/email', { email }),
  signup: (email: string, password: string, name: string) => post<SessionResult>('/signup', { email, password, name, locale: getLang() }, true),
  login: (email: string, password: string) => post<AuthResult>('/login', { email, password }, true),
  login2fa: (challenge: string, code: string) => post<SessionResult>('/login/2fa', { challenge, code }, true),
  forgot: (email: string) => post<{ sent: boolean }>('/forgot', { email }),
  reset: (email: string, code: string, password: string) => post<AuthResult>('/reset', { email, code, password }, true),
  account: () => api<Account>('/auth/account'),
  credentials: (email: string, password: string) => post<{ email_sent?: boolean; email_failed?: boolean }>('/credentials', { email, password }),
  /** A code to the new address; nothing changes until `confirmEmail`. */
  changeEmail: (email: string) => post<{ sent: boolean }>('/email/change', { email }),
  confirmEmail: (email: string, code: string) => post<{ email: string }>('/email/confirm', { email, code }),
  /** 2FA lost without recovery codes, at the sign-in's second step: a code by e-mail, then 2FA goes 24 h later. */
  mfaResetSend: (challenge: string) => post<{ sent: boolean; email: string }>('/2fa/reset/send', { challenge }),
  mfaResetConfirm: (challenge: string, code: string) => post<{ reset_at: number }>('/2fa/reset/confirm', { challenge, code }),
  mfaResetCancel: () => post<{ ok: boolean }>('/2fa/reset/cancel'),
  sendVerify: () => post<{ email_sent: boolean }>('/verify/send'),
  verify: (code: string) => post<{ email_verified: boolean }>('/verify', { code }),
  password: (body: { current?: string; password: string; revoke_others: boolean }) => post<{ ok: boolean; revoked: number }>('/password', body),
  setup2fa: () => post<{ secret: string; otpauth_url: string }>('/2fa/setup'),
  enable2fa: (code: string, revoke_others: boolean) => post<{ recovery_codes: string[] }>('/2fa/enable', { code, revoke_others }),
  disable2fa: (code: string) => post<{ ok: boolean }>('/2fa/disable', { code }),
  recovery: (code: string) => post<{ recovery_codes: string[] }>('/2fa/recovery', { code }),
  sessions: () => api<DeviceSession[]>('/auth/sessions'),
  revoke: (id: string) => api<{ ok: boolean }>(`/auth/sessions/${id}`, { method: 'DELETE' }),
  revokeOthers: () => post<{ revoked: number }>('/sessions/revoke-others'),
  logout: () => post<{ ok: boolean }>('/logout'),
  deleteAccount: (body: { password?: string; code?: string }) => api<{ ok: boolean }>('/auth/account', { method: 'DELETE', json: body }),
  /** Proves it is me (password, or a 2FA / recovery code): sensitive changes then work for 10 minutes. */
  reauth: (body: { password: string } | { code: string }) => post<{ ok: boolean }>('/reauth', body),
  /** A 60 s single-use stand-in for the token in a URL (the CSV link, the live socket). */
  ticket: (purpose: 'export' | 'live') => post<{ ticket: string }>('/ticket', { purpose }),
  oauthStart: (provider: Provider, app_redirect: string, mode: OAuthMode) =>
    post<{ url: string; key: string }>(`/oauth/${provider}/start`, mode === 'signin' ? { app_redirect } : { app_redirect, [mode]: true }),
  redeem: (code: string, key: string) => post<RedeemResult>('/oauth/redeem', { code, key }, true),
  unlink: (provider: Provider) => api<{ ok: boolean }>(`/auth/oauth/${provider}`, { method: 'DELETE' }),
};

/** `/api/features`, server-wide: kept when a room switch or a sign-in clears the cache (lib/query-client.ts). */
export const featuresQuery = { queryKey: ['features'], queryFn: () => api<string[]>('/features'), staleTime: Infinity };

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
    /** Le Tripot: bets with chips worth nothing (`/rounds/{id}/bet`, `/tripot`; docs/TRIPOT.md). */
    tripot: on.has('tripot'),
  };
}

export const accountKey = ['account'] as const;

/** My account (email, 2FA, linked providers); only asked of a server that has accounts. */
export function useAccount(enabled = true) {
  const { auth: hasAuth } = useFeatures();
  return useQuery({ queryKey: accountKey, queryFn: auth.account, enabled: enabled && hasAuth });
}

/** Between the sign-in screens (not in the URL): the email typed, the 2FA challenge. */
export const flow = { email: '', account: false, challenge: '', mailFailed: false };

/** An invite code (link or deep link) opened before signing in: joined right after. */
let pendingCode: string | null = presetJoinCode();
export const setPendingCode = (code: string | null) => {
  pendingCode = code?.trim().toUpperCase() || null;
};
export const pendingJoinCode = () => pendingCode;

/**
 * A shared question to play (`/play?share=…`) opened before signing in: back on /play right after.
 * On the web it is read at load (the router drops the query) and kept in the tab's session storage,
 * so leaving for Google / Discord and coming back keeps it.
 */
const PLAY_KEY = 'qdn.play';
const webSession = () => (Platform.OS === 'web' && typeof sessionStorage !== 'undefined' ? sessionStorage : null);
let pendingPlay: string | null = (() => {
  if (Platform.OS !== 'web' || typeof location === 'undefined') return null;
  const fromUrl = location.pathname.replace(/\/+$/, '') === '/play' ? new URLSearchParams(location.search).get('share') : null;
  if (fromUrl) webSession()?.setItem(PLAY_KEY, fromUrl);
  return fromUrl || webSession()?.getItem(PLAY_KEY) || null;
})();
export const setPendingPlay = (token: string | null) => {
  pendingPlay = token?.trim() || null;
  if (pendingPlay) webSession()?.setItem(PLAY_KEY, pendingPlay);
  else webSession()?.removeItem(PLAY_KEY);
};
export const pendingPlayToken = () => pendingPlay;

export const PASSWORD_MIN = 8;

export const looksLikeEmail = (v: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v.trim());

const wait = (s?: number) =>
  !s ? t('common.later') : s < 60 ? t('common.inSeconds', { count: s }) : t('common.inMinutes', { count: Math.ceil(s / 60) });

/** A short message for whatever went wrong, in the current language (`errors.<code>`, see locales/fr/errors.ts). */
export function authError(e: unknown): string {
  if (!(e instanceof ApiError)) return t('common.offline');
  if (e.code === 'rate_limited') return t('common.tooManyTries', { when: wait(e.retryAfter) });
  return Object.prototype.hasOwnProperty.call(errors, e.code)
    ? t(`errors.${e.code as keyof typeof errors}`, { min: PASSWORD_MIN })
    : t('common.failed');
}
