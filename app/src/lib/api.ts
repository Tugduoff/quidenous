import { Platform } from 'react-native';

import { deviceHeader } from './device';

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

const ROOM_PARAM =
  Platform.OS === 'web' && typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get('room')) || null : null;

/** Room carried by a tapped notification (`/round/12?room=3`): the app opens in it. */
export const presetRoom = () => ROOM_PARAM;

/** Friend code carried by a friend link `/?friend=…` (QR code). */
export const presetFriendCode = () => FRIEND_CODE;

/** Group code baked into the build (CI secret) or carried by the invite link `/?code=…`. */
export function presetJoinCode(): string | null {
  // Rooms are open to anyone now: only an invite link fills the code in (no group baked into the build).
  return INVITE_CODE;
}

export let API_URL = DEFAULT_API_URL;
export const setApiUrl = (url: string) => {
  API_URL = url.trim().replace(/\/+$/, '');
};

/** `left`: someone who left or was removed (kept for names and photos in the history). */
export type Member = { id: number; name: string; color: string; photo_url: string | null; left?: boolean };
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
  /** `vibe`: the built-in deck's tone (null = none; absent from an older server). */
  settings: { skip_vote: boolean; kick_vote: boolean; questions_by: 'everyone' | 'owner'; vibe?: string | null };
  /** Current members only. */
  members: (Member & { role: Role; user_id: number })[];
  /** People who left or were removed (absent from an older server). */
  former?: (Member & { user_id?: number | null })[];
  kicks: VoteCount[];
  kick_needed: number;
};
/** A tone for a room's built-in questions; `icon` is a lucide name in kebab-case. */
export type Vibe = { id: string; name: string; icon: string; description: string };
export type Friend = { user_id: number; name: string; color: string; photo_url: string | null; rooms_in_common: number; added: boolean };
export type PackSummary = { id: number; name: string; description: string; author: string; installs: number; count: number; mine: boolean; sample: string[] };
export type Pack = Omit<PackSummary, 'count' | 'sample'> & { questions: { text: string; kind: 'pick' | 'rank' }[] };
/** `muted`: no pushes from this room for me (absent from an older server). */
export type RoomSummary = { id: number; name: string; role: Role; members: number; open_number: number | null; voted: boolean; unread: number; muted?: boolean };
export type HistoryItem = {
  id: number;
  number: number;
  question: string;
  closed_at: string;
  winner_ids: number[];
  top_count: number;
  vote_count: number;
  pinned: boolean;
  kind: 'pick' | 'rank';
};
/** `user_id`: the author's account (null once deleted; absent from an older server). */
export type Message = { id: number; member_id: number; user_id?: number | null; text: string; created_at: string };
export type InboxItem = {
  id: number;
  /** Unknown kinds (a newer server) are shown with a generic icon and title. */
  kind: 'message' | 'nudge' | 'result' | 'question' | 'added' | 'ask' | (string & {});
  round_id: number | null;
  /** The room it happened in (absent from an older server). */
  room_id?: number;
  actor_id: number | null;
  text: string;
  created_at: string;
  read: boolean;
};
/** `total`: the denominator, ratio awards only. */
export type Award = { member_id: number; value: number; total?: number };
export type Duo = { voter_id: number; target_id: number; value: number; total?: number };
/** Two voters; `a_id < b_id`. */
export type Pair = { a_id: number; b_id: number; value: number; total: number };
/** `avg`: average position on ranking questions (1 decimal). */
export type RankAward = { member_id: number; avg: number; count: number };
export type QuestionRef = { round_id: number; number: number; question: string; kind: 'pick' | 'rank'; value: number; total: number };
export type StatsPeriod = '7d' | '30d' | 'all';
/** A highlight of the period, ready to show: `icon` is a lucide name in kebab-case, `line` already has the names in it. */
export type Story = { key: string; icon: string; title: string; big: string; line: string; member_ids: number[]; round_id?: number };
export type MemberStats = {
  member_id: number;
  present: number;
  votes_cast: number;
  received: number;
  wins: number;
  win_rate: number;
  messages: number;
  self_votes: number;
  median_vote_seconds: number | null;
  avg_rank: number | null;
  top_voters: { member_id: number; value: number }[];
  top_targets: { member_id: number; value: number }[];
  best: QuestionRef | null;
  worst: QuestionRef | null;
  /** Award keys held this period. */
  titles: string[];
};
export type Activity = { bucket: 'day' | 'week'; start: string; questions: number[]; votes: number[] };
/**
 * `GET /api/stats` (docs/STATS.md). Everything after `never_chosen` is newer: an older server leaves it out,
 * `normalizeStats` fills it in (null awards, empty lists) so the screen only ever hides what is missing.
 */
export type Stats = {
  period: StatsPeriod;
  members: number;
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
  duo: Duo | null;
  never_chosen: number[];
  /** null: an older server that doesn't count them. */
  messages: number | null;
  contrarian: Award | null;
  sheep: Award | null;
  loyal: Duo | null;
  twins: Pair | null;
  kingmaker: Award | null;
  streak: Award | null;
  longest_streak: Award | null;
  comeback: Award | null;
  night_owl: Award | null;
  early_bird: Award | null;
  chatterbox: Award | null;
  silent: Award | null;
  best_rank: RankAward | null;
  worst_rank: RankAward | null;
  most_split: QuestionRef | null;
  most_unanimous: QuestionRef | null;
  hottest: QuestionRef | null;
  stories: Story[];
  /** Who votes for whom, self included, non-zero only. */
  edges: { voter_id: number; target_id: number; value: number }[];
  /** [7][24] votes cast, weekday 0 = Monday, local hours (empty from an older server). */
  heatmap: number[][];
  activity: Activity | null;
  profiles: MemberStats[];
};

const STATS_NULLABLE = [
  'contrarian', 'sheep', 'loyal', 'twins', 'kingmaker', 'streak', 'longest_streak', 'comeback', 'night_owl', 'early_bird',
  'chatterbox', 'silent', 'best_rank', 'worst_rank', 'most_split', 'most_unanimous', 'hottest', 'most_chosen', 'most_crowned',
  'narcissist', 'fastest', 'slowest', 'last_voter', 'nudger', 'most_nudged', 'author', 'duo', 'median_round_seconds',
] as const;

const list = <T,>(v: unknown): T[] => (Array.isArray(v) ? (v as T[]) : []);
const num = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

/** Whatever the server sent (an older one has fewer fields), as a complete `Stats`. */
export function normalizeStats(raw: Partial<Stats> | null, period: StatsPeriod): Stats {
  const r = (raw ?? {}) as Record<string, unknown>;
  const s = { ...r } as Record<string, unknown>;
  for (const k of STATS_NULLABLE) s[k] = r[k] ?? null;
  for (const k of ['members', 'questions', 'votes', 'unanimous', 'ties', 'nudges', 'seconds_voting', 'seconds_waiting_last']) s[k] = num(r[k]);
  s.period = r.period ?? period;
  s.messages = typeof r.messages === 'number' ? r.messages : null;
  s.never_chosen = list<number>(r.never_chosen);
  s.stories = list<Story>(r.stories).filter((x) => x && typeof x.key === 'string');
  s.edges = list(r.edges);
  const heat = list<number[]>(r.heatmap);
  s.heatmap = heat.length === 7 && heat.every((row) => Array.isArray(row) && row.length === 24) ? heat : [];
  const a = r.activity as Activity | undefined;
  s.activity = a && Array.isArray(a.questions) ? { ...a, votes: list<number>(a.votes) } : null;
  s.profiles = list<MemberStats>(r.profiles).map((p) => ({
    ...p,
    top_voters: list(p.top_voters),
    top_targets: list(p.top_targets),
    titles: list<string>(p.titles),
    best: p.best ?? null,
    worst: p.worst ?? null,
    median_vote_seconds: p.median_vote_seconds ?? null,
    avg_rank: p.avg_rank ?? null,
  }));
  return s as Stats;
}
export type Queue = {
  custom: { id: number; text: string; author_id: number | null; kind: 'pick' | 'rank' }[];
  deck_left: number;
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

/** Called when the server no longer knows my token (signed out from another device): back to the welcome screen. */
let onSignedOut: (() => void) | null = null;
export const setOnSignedOut = (f: (() => void) | null) => {
  onSignedOut = f;
};

async function fail(res: Response, sentToken: boolean): Promise<never> {
  const data = await res.json().catch(() => null);
  const code = data?.error ?? 'unknown';
  if (res.status === 401 && code === 'unauthorized' && sentToken) onSignedOut?.();
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
const roomQuery = () => (room != null ? `&room=${room}` : '');

/** Plain link to the CSV of the whole history (the token rides in the query so a browser can download it). */
export const historyExportUrl = () => `${API_URL}/api/history/export?token=${token ?? ''}${roomQuery()}`;

export async function api<T>(path: string, init: RequestInit & { json?: unknown } = {}): Promise<T> {
  const headers: Record<string, string> = { ...deviceHeader(), ...(init.headers as Record<string, string>) };
  if (token) headers.authorization = `Bearer ${token}`;
  if (room != null) headers['x-room'] = String(room);
  let body = init.body;
  if (init.json !== undefined) {
    headers['content-type'] = 'application/json';
    body = JSON.stringify(init.json);
  }
  const res = await fetch(`${API_URL}/api${path}`, { ...init, headers, body });
  if (!res.ok) return fail(res, !!token);
  return (await res.json().catch(() => null)) as T;
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
  if (!res.ok) return fail(res, !!token);
  return (await res.json().catch(() => null)) as T;
}
