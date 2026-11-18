import { Platform } from 'react-native';
import { deviceHeader } from '@/lib/device/info';
import { appVersion } from '@/lib/device/update';
import type { Member } from '@/lib/api/group';

/** Same origin on web (served by the Rust server); on phones, the build URL or the one typed at join. */
export const DEFAULT_API_URL =
  process.env.EXPO_PUBLIC_API_URL ??
  (Platform.OS === 'web' && typeof location !== 'undefined' ? location.origin : '');
export const SERVER_FIELD = Platform.OS !== 'web' && !process.env.EXPO_PUBLIC_API_URL;

export let API_URL = DEFAULT_API_URL;
export const setApiUrl = (url: string) => {
  API_URL = url.trim().replace(/\/+$/, '');
};

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    /** Seconds to wait, on `429 rate_limited`. */
    public retryAfter?: number,
  ) {
    super(code);
  }
}

/** The server says this thing doesn't exist (or no longer): no point retrying. */
export const isNotFound = (e: unknown) => e instanceof ApiError && e.status === 404;

/** Called when the server no longer knows my token (signed out from another device): back to the welcome screen. */
let onSignedOut: (() => void) | null = null;
export const setOnSignedOut = (f: (() => void) | null) => {
  onSignedOut = f;
};

/** Called when the server no longer serves this version of the app (`426 update_required`, see `MIN_APP_VERSION`). */
let onUpdateRequired: (() => void) | null = null;
export const setOnUpdateRequired = (f: (() => void) | null) => {
  onUpdateRequired = f;
};

async function fail(res: Response, sentToken: boolean): Promise<never> {
  const data = await res.json().catch(() => null);
  const code = data?.error ?? 'unknown';
  if (res.status === 401 && code === 'unauthorized' && sentToken) onSignedOut?.();
  if (res.status === 426) onUpdateRequired?.();
  const retry = Number(data?.retry_after ?? res.headers.get('retry-after')) || undefined;
  throw new ApiError(res.status, code, retry);
}

let token: string | null = null;
export const setApiToken = (value: string | null) => {
  token = value;
};

/** The room every request is about (`X-Room`); null = the one I used last. */
let room: number | null = null;
export const setApiRoom = (value: number | null) => {
  room = value;
};
export const apiRoom = () => room;
export const roomQuery = () => (room != null ? `&room=${room}` : '');

/**
 * How a plain link or a socket (no `Authorization` header) says who I am: a 60 s single-use ticket when the server has
 * them (`tickets` in /api/features), so the token never sits in a URL; the token itself for an older server.
 */
export async function urlAuth(purpose: 'export' | 'live', tickets: boolean, own = token): Promise<string> {
  if (!tickets) return `token=${own ?? ''}`;
  const { ticket } = await api<{ ticket: string }>('/auth/ticket', { method: 'POST', json: { purpose } });
  return `ticket=${ticket}`;
}

/** A request with no answer after this long fails (a captive Wi-Fi or a tunnel would otherwise hang forever). */
const TIMEOUT_MS = 15_000;
/** Uploads (a photo on a slow network) get longer. */
const UPLOAD_TIMEOUT_MS = 60_000;

/** `fetch` that gives up after `ms` (a network error for the caller, like being offline). */
async function fetchWithin(url: string, init: RequestInit, ms: number): Promise<Response> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), ms);
  const outer = init.signal;
  if (outer) {
    if (outer.aborted) controller.abort();
    else outer.addEventListener('abort', () => controller.abort());
  }
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

/** `room`: this call is about another room than the current one (`X-Room` for it alone). */
export async function api<T>(path: string, init: RequestInit & { json?: unknown; room?: number } = {}): Promise<T> {
  // `x-kinds`: this app answers choose and said questions (an older one gets them as a plain pick).
  // `x-app-version`: a server that no longer serves it answers 426 (the app asks to update).
  const headers: Record<string, string> = {
    ...deviceHeader(),
    'x-kinds': 'choose,said',
    'x-app-version': appVersion(),
    ...(init.headers as Record<string, string>),
  };
  if (token) headers.authorization = `Bearer ${token}`;
  const target = init.room ?? room;
  if (target != null) headers['x-room'] = String(target);
  let body = init.body;
  if (init.json !== undefined) {
    headers['content-type'] = 'application/json';
    body = JSON.stringify(init.json);
  }
  const res = await fetchWithin(`${API_URL}/api${path}`, { ...init, headers, body }, TIMEOUT_MS);
  if (!res.ok) return fail(res, !!token);
  return (await res.json().catch(() => null)) as T;
}

export const photoUrl = (m: Member) => (m.photo_url ? `${API_URL}${m.photo_url}` : null);

/** A media path from the server (`/api/…`) on the server in use; an absolute URL as is. */
export const mediaUrl = (url: string) => (url.startsWith('/') ? `${API_URL}${url}` : url);

/** Multipart upload (photo). Content type is set by fetch with the boundary. */
export async function apiUpload<T>(path: string, form: FormData, method = 'PUT'): Promise<T> {
  const res = await fetchWithin(`${API_URL}/api${path}`, {
    method,
    headers: {
      'x-kinds': 'choose,said',
      'x-app-version': appVersion(),
      ...(token ? { authorization: `Bearer ${token}`, ...(room != null ? { 'x-room': String(room) } : {}) } : {}),
    },
    body: form,
  }, UPLOAD_TIMEOUT_MS);
  if (!res.ok) return fail(res, !!token);
  return (await res.json().catch(() => null)) as T;
}
