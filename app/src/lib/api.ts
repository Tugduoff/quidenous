import { Platform } from 'react-native';

/** Same origin on web (served by the Rust server); on phones, the build URL or the one typed at join. */
export const DEFAULT_API_URL =
  process.env.EXPO_PUBLIC_API_URL ??
  (Platform.OS === 'web' && typeof location !== 'undefined' ? location.origin : '');
export const SERVER_FIELD = Platform.OS !== 'web' && !process.env.EXPO_PUBLIC_API_URL;

// Read at load time: the router rewrites the URL (and drops the query) once it mounts.
const INVITE_CODE =
  Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('code')
    : null;

const FRIEND_CODE =
  Platform.OS === 'web' && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('friend') : null;

/** Friend code carried by a friend link `/?friend=…` (QR code). */
export const presetFriendCode = () => FRIEND_CODE;

/** Group code baked into the build (CI secret) or carried by the invite link `/?code=…`. */
export function presetJoinCode(): string | null {
  return process.env.EXPO_PUBLIC_JOIN_CODE || INVITE_CODE;
}

export let API_URL = DEFAULT_API_URL;
export const setApiUrl = (url: string) => {
  API_URL = url.trim().replace(/\/+$/, '');
};

export type Member = { id: number; name: string; color: string; photo_url: string | null };
export type Tally = { target_id: number; count: number; voter_ids: number[] };
export type Round = {
  id: number;
  number: number;
  question: string;
  custom: boolean;
  /** 'pick' one person, or 'rank' everyone from first to last. */
  kind: 'pick' | 'rank';
  opened_at: string;
  closed_at: string | null;
  member_count: number;
  voter_ids: number[];
  my_vote: number | null;
  result: Tally[] | null;
  pinned: boolean;
  /** Open question only: who agreed not to wait for whom. */
  skips: VoteCount[];
  skip_needed: number;
  skipped_ids: number[];
  ranking: RankEntry[] | null;
  my_ranking: number[] | null;
};
export type RankEntry = { member_id: number; points: number; average: number; firsts: number };
/** A count of agreements (skip someone, remove someone), and whether mine is one of them. */
export type VoteCount = { target_id: number; count: number; mine: boolean };
export type Role = 'owner' | 'member';
export type Group = {
  id: number;
  name: string;
  code: string;
  my_role: Role;
  settings: { skip_vote: boolean; kick_vote: boolean; questions_by: 'everyone' | 'owner' };
  members: (Member & { role: Role; user_id: number })[];
  kicks: VoteCount[];
  kick_needed: number;
};
export type Friend = { user_id: number; name: string; color: string; photo_url: string | null; rooms_in_common: number; added: boolean };
export type RoomSummary = { id: number; name: string; role: Role; members: number; open_number: number | null; voted: boolean; unread: number };
export type HistoryItem = {
  id: number;
  number: number;
  question: string;
  closed_at: string;
  winner_ids: number[];
  top_count: number;
  vote_count: number;
  pinned: boolean;
};
export type Message = { id: number; member_id: number; text: string; created_at: string };
export type InboxItem = {
  id: number;
  kind: 'message' | 'nudge' | 'result' | 'question';
  round_id: number | null;
  actor_id: number | null;
  text: string;
  created_at: string;
  read: boolean;
};
export type Award = { member_id: number; value: number };
export type Stats = {
  questions: number;
  votes: number;
  unanimous: number;
  ties: number;
  nudges: number;
  /** Estimate: 20 s per vote. */
  seconds_voting: number;
  seconds_waiting_last: number;
  median_round_seconds: number | null;
  most_chosen: Award | null;
  most_crowned: Award | null;
  narcissist: Award | null;
  fastest: Award | null;
  slowest: Award | null;
  last_voter: Award | null;
  nudger: Award | null;
  most_nudged: Award | null;
  author: Award | null;
  duo: { voter_id: number; target_id: number; value: number } | null;
  never_chosen: number[];
};
export type Queue = {
  custom: { id: number; text: string; author_id: number | null; kind: 'pick' | 'rank' }[];
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

/** The room every request is about (`X-Room`); null = the one I used last. */
let room: number | null = null;
export const setApiRoom = (value: number | null) => {
  room = value;
};
export const apiRoom = () => room;
const roomQuery = () => (room != null ? `&room=${room}` : '');

/** Plain link to the CSV of the whole history (the token rides in the query so a browser can download it). */
export const historyExportUrl = () => `${API_URL}/api/history/export?token=${token ?? ''}${roomQuery()}`;

export async function api<T>(path: string, init: RequestInit & { json?: unknown } = {}): Promise<T> {
  const headers: Record<string, string> = { ...(init.headers as Record<string, string>) };
  if (token) headers.authorization = `Bearer ${token}`;
  if (room != null) headers['x-room'] = String(room);
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

export const liveUrl = (t: string) => `${API_URL.replace(/^http/, 'ws')}/api/live?token=${t}${roomQuery()}`;

/** Multipart upload (photo). Content type is set by fetch with the boundary. */
export async function apiUpload<T>(path: string, form: FormData, method = 'PUT'): Promise<T> {
  const res = await fetch(`${API_URL}/api${path}`, {
    method,
    headers: token ? { authorization: `Bearer ${token}`, ...(room != null ? { 'x-room': String(room) } : {}) } : undefined,
    body: form,
  });
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new ApiError(res.status, data?.error ?? 'unknown');
  return data as T;
}
