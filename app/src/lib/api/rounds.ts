import type { VoteCount } from '@/lib/api/group';
import type { Tripot } from '@/lib/api/tripot';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

export const roundKeys = {
  all: ['round'] as const,
  current: ['round', 'current'] as const,
  unseen: ['round', 'unseen'] as const,
  detail: (id: number) => ['round', id] as const,
};

export type Tally = { target_id: number; count: number; voter_ids: number[] };

export type Round = {
  id: number;
  number: number;
  question: string;
  custom: boolean;
  /** 'pick' one person, 'rank' everyone from first to last, or 'choose' an item for everyone (docs/CHOOSE.md). */
  kind: QuestionKind;
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
  /** Missed the last few questions in a row: not waited for until they vote again (absent from an older server). */
  away_ids?: number[];
  ranking: RankEntry[] | null;
  my_ranking: number[] | null;
  /** Choose question: its items (null otherwise; absent from an older server). */
  items: string[] | null;
  /** Choose question: my wires (`item` = index in `items`), once I voted. */
  my_choices: Choice[] | null;
  /** Choose question, closed: per person what the others guessed and what they took, and who guessed best. */
  choose: ChooseResult | null;
  /** A shared-deck question that came from someone's suggestion: their name (absent from an older server). */
  suggested_by?: string | null;  /** Anonymous votes (docs/ANONYMOUS.md): `result` has counts only, empty `voter_ids` (absent from an older server). */
  anonymous?: boolean;
  /** Le Tripot (docs/TRIPOT.md): bets on the winner; absent where it doesn't apply (and from an older server). */
  tripot?: Tripot;
  /** « Qui a dit ça ? » question (docs/SAID.md): its phase, answers and result (null otherwise; absent from an older server). */
  said?: SaidView | null;
};

export type QuestionKind = 'pick' | 'rank' | 'choose' | 'said';

/** One answer of a said question. `text` null: hidden by reports (`hidden`), or a pass from an older app (not guessable). */
export type SaidAnswer = { id: number; text: string | null; hidden: boolean };

/** A guess: answer `answer_id` was written by `member_id`. */
export type SaidGuess = { answer_id: number; member_id: number };

/** Done only. `answers` in reveal order (most found first, best hidden last); `scores` best first. */
export type SaidResult = {
  answers: { id: number; author_id: number; found: number; fooled: number; guesses: { voter_id: number; member_id: number }[] }[];
  scores: { member_id: number; points: number; found: number; fooled: number }[];
};

/** `RoundView.said` (docs/SAID.md): writing, then guessing who wrote what, then the reveal. */
export type SaidView = {
  phase: 'writing' | 'guessing' | 'done';
  guess_at: string | null;
  /** Who has written (passes included), member order. */
  writer_ids: number[];
  my_answer: { id: number; text: string } | null;
  /** Guessing and done: every real answer in a random fixed order, mine included. */
  answers: SaidAnswer[] | null;
  /** Once I sent them. */
  my_guesses: SaidGuess[] | null;
  result: SaidResult | null;
};

/** A wire: this person (`member_id`) takes `items[item]`. */
export type Choice = { member_id: number; item: number };

/**
 * `people` in member order: `real` their own wire (null if they didn't wire), `guesses` the others' wires to them,
 * `top` the most guessed item(s). `scores`: each voter's guesses that matched (`hits`) out of those that could (`total`), best first.
 */
export type ChooseResult = {
  people: { member_id: number; real: number | null; top: number[]; guesses: { voter_id: number; item: number }[] }[];
  scores: { member_id: number; hits: number; total: number }[];
};

export type RankEntry = { member_id: number; points: number; average: number; firsts: number };

export const useCurrentRound = () =>
  useQuery({ queryKey: roundKeys.current, queryFn: () => api<Round | null>('/round/current') });

/**
 * A finished question I voted on but have not seen the result of (I was away when it closed): the oldest, with how many
 * are waiting and the newest of them (marking that one seen skips them all). Both absent from an older server.
 */
export type UnseenRound = Round & { unseen_count?: number; unseen_last_id?: number };

export const useUnseenRound = () =>
  useQuery({ queryKey: roundKeys.unseen, queryFn: () => api<UnseenRound | null>('/round/unseen') });

export const useRound = (id: number | null) =>
  useQuery({ queryKey: roundKeys.detail(id ?? 0), queryFn: () => api<Round>(`/rounds/${id}`), enabled: id != null });
