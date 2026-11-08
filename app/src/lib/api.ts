import { Platform } from 'react-native';

/** Same origin on web (served by the Rust server), configured URL on phones. */
export const API_URL =
  process.env.EXPO_PUBLIC_API_URL ??
  (Platform.OS === 'web' && typeof location !== 'undefined' ? location.origin : 'http://localhost:8080');

export type Member = { id: number; name: string; color: string; photo_url: string | null };
export type Tally = { target_id: number; count: number; voter_ids: number[] };
export type Round = {
  id: number;
  number: number;
  question: string;
  custom: boolean;
  opened_at: string;
  closed_at: string | null;
  member_count: number;
  voter_ids: number[];
  my_vote: number | null;
  result: Tally[] | null;
  my_rating: number | null;
};
export type HistoryItem = {
  id: number;
  number: number;
  question: string;
  closed_at: string;
  winner_ids: number[];
  top_count: number;
  vote_count: number;
};
export type Queue = {
  custom: { id: number; text: string; author_id: number | null }[];
  deck_left: number;
};

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
  ) {
    super(code);
  }
}

let token: string | null = null;
export const setApiToken = (value: string | null) => {
  token = value;
};

export async function api<T>(path: string, init: RequestInit & { json?: unknown } = {}): Promise<T> {
  const headers: Record<string, string> = { ...(init.headers as Record<string, string>) };
  if (token) headers.authorization = `Bearer ${token}`;
  let body = init.body;
  if (init.json !== undefined) {
    headers['content-type'] = 'application/json';
    body = JSON.stringify(init.json);
  }
  const res = await fetch(`${API_URL}/api${path}`, { ...init, headers, body });
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new ApiError(res.status, data?.error ?? 'unknown');
  return data as T;
}

export const photoUrl = (m: Member) => (m.photo_url ? `${API_URL}${m.photo_url}` : null);

export const liveUrl = (t: string) => `${API_URL.replace(/^http/, 'ws')}/api/live?token=${t}`;
