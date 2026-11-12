import { useQuery } from '@tanstack/react-query';

import { API_URL, api, ApiError, presetJoinCode } from './api';
import { deviceHeader } from './device';

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
  signup: (email: string, password: string, name: string) => post<SessionResult>('/signup', { email, password, name }, true),
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

export const PASSWORD_MIN = 8;

export const looksLikeEmail = (v: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v.trim());

const MESSAGES: Record<string, string> = {
  bad_email: 'E-mail invalide',
  bad_name: 'Prénom invalide',
  weak_password: `${PASSWORD_MIN} caractères minimum`,
  common_password: 'Mot de passe trop courant',
  email_taken: 'E-mail déjà utilisé',
  bad_credentials: 'Mot de passe incorrect',
  bad_code: 'Code incorrect',
  expired: 'Code expiré',
  email_not_configured: "Envoi d'e-mails indisponible",
  already_secured: 'Compte déjà sécurisé',
  already_verified: 'E-mail déjà vérifié',
  no_email: "Ajoute d'abord un e-mail",
  no_setup: 'Recommence la configuration',
  identity_taken: 'Ce compte est déjà lié à un autre profil',
  last_method: 'Garde au moins un moyen de connexion',
  unknown_provider: 'Connexion indisponible',
  bad_redirect: 'Connexion indisponible',
  account_protected: 'Ce prénom a un compte : connecte-toi',
  claimed_session: 'Fais-le depuis ton premier appareil.',
  sign_in_required: 'Connecte-toi pour continuer',
};

const wait = (s?: number) => (!s ? 'plus tard' : s < 60 ? `dans ${s} s` : `dans ${Math.ceil(s / 60)} min`);

/** A short French message for whatever went wrong. */
export function authError(e: unknown): string {
  if (!(e instanceof ApiError)) return 'Pas de connexion';
  if (e.code === 'rate_limited') return `Trop d'essais, réessaie ${wait(e.retryAfter)}`;
  return MESSAGES[e.code] ?? 'Raté, réessaie';
}
