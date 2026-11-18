import { useQuery, keepPreviousData } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

export const statsKeys = {
  all: ['stats'] as const,
  period: (period: StatsPeriod) => ['stats', period] as const,
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
  /** Days in a row with a vote, all time (docs/STREAKS.md): absent from an older server and in a party. */
  day_streaks?: { member: Award | null; room: { current: number; best: number } };
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

/** Stats of a period, in the phone's time zone (`enabled`: only once needed). Switching period keeps the previous numbers on screen until the new ones land. */
export const useStats = (period: StatsPeriod, enabled = true) =>
  useQuery({
    queryKey: statsKeys.period(period),
    enabled,
    queryFn: async () => normalizeStats(await api<Partial<Stats> | null>(`/stats?period=${period}&tz=${-new Date().getTimezoneOffset()}`), period),
    placeholderData: keepPreviousData,
  });

export const snapshotStory = (period: StatsPeriod, key: string) =>
  api<{ token: string; path: string }>('/stats/share', { method: 'POST', json: { period, key, tz: -new Date().getTimezoneOffset() } });
