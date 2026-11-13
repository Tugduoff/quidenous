import { useQuery } from '@tanstack/react-query';
import { Platform } from 'react-native';

import { errors } from '@/locales/fr/errors';

import { API_URL, api, ApiError, presetJoinCode } from './api';
import { deviceHeader } from './device';
import { getLang, t } from './i18n';

export type Provider = 'google' | 'discord';
export const PROVIDERS: Provider[] = ['google', 'discord'];
export const PROVIDER_NAMES: Record<Provider, string> = { google: 'Google', discord: 'Discord' };

/** A new session (`email_sent` only on sign-up). */
export type SessionResult = { token: string; user_id: number; new: boolean; email_sent?: boolean };
/** 2FA is on: trade the challenge and a code for the session. */
export type Challenge = { mfa: 'totp'; challenge: string };
export type AuthResult = SessionResult | Challenge;
export type RedeemResult = AuthResult | { linked: Provider };

export const isChallenge = (r: RedeemResult): r is Challenge => 'challenge' in r;
export const isLinked = (r: RedeemResult): r is { linked: Provider } => 'linked' in r;

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
  credentials: (email: string, password: string) => post<{ email_sent?: boolean }>('/credentials', { email, password }),
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
  oauthStart: (provider: Provider, app_redirect: string, link: boolean) =>
    post<{ url: string; key: string }>(`/oauth/${provider}/start`, link ? { app_redirect, link } : { app_redirect }),
  redeem: (code: string, key: string) => post<RedeemResult>('/oauth/redeem', { code, key }, true),
  unlink: (provider: Provider) => api<{ ok: boolean }>(`/auth/oauth/${provider}`, { method: 'DELETE' }),
};

/** What the server can do (`/api/features`). An older server has no "auth": the app keeps the old join screen. */
export function useFeatures() {
  // No server known yet (phone build without a baked-in URL): the old screen, which asks for one.
  const known = !!API_URL;
  const { data, isError } = useQuery({
    queryKey: ['features'],
    queryFn: () => api<string[]>('/features'),
    staleTime: Infinity,
    retry: 2,
    enabled: known,
  });
  const on = new Set(data ?? []);
  return {
    loaded: !known || !!data || isError,
    auth: on.has('auth'),
    email: on.has('email'),
    providers: PROVIDERS.filter((p) => on.has(`oauth_${p}`)),
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
  };
}

export const accountKey = ['account'] as const;

/** My account (email, 2FA, linked providers); only asked of a server that has accounts. */
export function useAccount(enabled = true) {
  const { auth: hasAuth } = useFeatures();
  return useQuery({ queryKey: accountKey, queryFn: auth.account, enabled: enabled && hasAuth });
}

/** Between the sign-in screens (not in the URL): the email typed, the 2FA challenge. */
export const flow = { email: '', account: false, challenge: '' };

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
