import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { deviceHeader } from '@/lib/device/info';
import { getLang } from '@/lib/i18n';
import { useFeatures } from '@/lib/api/features';
import { type Member } from '@/lib/api/group';

export type Provider = 'google' | 'discord';
export const PROVIDERS: Provider[] = ['google', 'discord'];
export const PROVIDER_NAMES: Record<Provider, string> = { google: 'Google', discord: 'Discord' };

export type SessionResult = { token: string; user_id: number; new: boolean; email_sent?: boolean; email_failed?: boolean };
export type Challenge = { mfa: 'totp'; challenge: string };
export type AuthResult = SessionResult | Challenge;
export type RedeemResult = AuthResult | { linked: Provider } | { reauth: true };

export const isChallenge = (r: RedeemResult): r is Challenge => 'challenge' in r;
export const isLinked = (r: RedeemResult): r is { linked: Provider } => 'linked' in r;
export const isReauth = (r: RedeemResult): r is { reauth: true } => 'reauth' in r;

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
  guest?: boolean;
  mfa_reset_at?: number | null;
  terms?: boolean;
};

export type DeviceSession = { id: string; device: string | null; created_at: string; last_used_at: string | null; current: boolean };

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
  changeEmail: (email: string) => post<{ sent: boolean }>('/email/change', { email }),
  confirmEmail: (email: string, code: string) => post<{ email: string }>('/email/confirm', { email, code }),
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
  reauth: (body: { password: string } | { code: string }) => post<{ ok: boolean }>('/reauth', body),
  ticket: (purpose: 'export' | 'live') => post<{ ticket: string }>('/ticket', { purpose }),
  oauthStart: (provider: Provider, app_redirect: string, mode: OAuthMode) =>
    post<{ url: string; key: string }>(`/oauth/${provider}/start`, mode === 'signin' ? { app_redirect } : { app_redirect, [mode]: true }),
  redeem: (code: string, key: string) => post<RedeemResult>('/oauth/redeem', { code, key }, true),
  unlink: (provider: Provider) => api<{ ok: boolean }>(`/auth/oauth/${provider}`, { method: 'DELETE' }),
};

export const authKeys = {
  account: ['account'] as const,
  sessions: ['sessions'] as const,
};

export function useAccount(enabled = true) {
  const { auth: hasAuth } = useFeatures();
  return useQuery({ queryKey: authKeys.account, queryFn: auth.account, enabled: enabled && hasAuth });
}

export const useDeviceSessions = () => useQuery({ queryKey: authKeys.sessions, queryFn: auth.sessions });

export const acceptTerms = () => api('/auth/terms', { method: 'POST' });

export type LegacyEntry = { token: string; member: Member; room_id: number };

export const joinLegacy = (code: string, name: string) =>
  api<LegacyEntry>('/join', { method: 'POST', json: { code, name }, headers: deviceHeader() });

export const startLegacy = (name: string, room: string) =>
  api<LegacyEntry>('/start', { method: 'POST', json: { name, room }, headers: deviceHeader() });
