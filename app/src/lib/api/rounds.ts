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
  kind: QuestionKind;
  opened_at: string;
  closed_at: string | null;
  member_count: number;
  voter_ids: number[];
  my_vote: number | null;
  result: Tally[] | null;
  pinned: boolean;
  skips: VoteCount[];
  skip_needed: number;
  skipped_ids: number[];
  away_ids?: number[];
  ranking: RankEntry[] | null;
  my_ranking: number[] | null;
  items: string[] | null;
  my_choices: Choice[] | null;
  choose: ChooseResult | null;
  suggested_by?: string | null;
  anonymous?: boolean;
  tripot?: Tripot;
  said?: SaidView | null;
};

export type QuestionKind = 'pick' | 'rank' | 'choose' | 'said';

export type SaidAnswer = { id: number; text: string | null; hidden: boolean };

export type SaidGuess = { answer_id: number; member_id: number };

export type SaidResult = {
  answers: { id: number; author_id: number; found: number; fooled: number; guesses: { voter_id: number; member_id: number }[] }[];
  scores: { member_id: number; points: number; found: number; fooled: number }[];
};

export type SaidView = {
  phase: 'writing' | 'guessing' | 'done';
  guess_at: string | null;
  writer_ids: number[];
  my_answer: { id: number; text: string } | null;
  answers: SaidAnswer[] | null;
  my_guesses: SaidGuess[] | null;
  result: SaidResult | null;
};

export type Choice = { member_id: number; item: number };

export type ChooseResult = {
  people: { member_id: number; real: number | null; top: number[]; guesses: { voter_id: number; item: number }[] }[];
  scores: { member_id: number; hits: number; total: number }[];
};

export type RankEntry = { member_id: number; points: number; average: number; firsts: number };

export const getCurrentRound = (room?: number) => api<Round | null>('/round/current', { room });

export const useCurrentRound = () => useQuery({ queryKey: roundKeys.current, queryFn: () => getCurrentRound() });

export type UnseenRound = Round & { unseen_count?: number; unseen_last_id?: number };

export const getUnseenRound = (room?: number) => api<UnseenRound | null>('/round/unseen', { room });

export const useUnseenRound = () => useQuery({ queryKey: roundKeys.unseen, queryFn: () => getUnseenRound() });

export const getRound = (id: number, room?: number) => api<Round>(`/rounds/${id}`, { room });

export const useRound = (id: number | null) =>
  useQuery({ queryKey: roundKeys.detail(id ?? 0), queryFn: () => (id == null ? Promise.reject(new Error('no round')) : getRound(id)), enabled: id != null });

export type Ballot = { target_id: number } | { ranking: number[] } | { choices: Choice[] } | { guesses: SaidGuess[] };

export const voteCurrent = (ballot: Ballot) => api('/round/current/vote', { method: 'POST', json: ballot });

export const answerCurrent = (text: string) => api<Round>('/round/current/answer', { method: 'POST', json: { text } });

export const nudgeMember = (memberId: number) => api('/round/current/nudge', { method: 'POST', json: { member_id: memberId } });

export const setSkip = (memberId: number, on: boolean) => api(`/round/current/skip/${memberId}`, { method: on ? 'PUT' : 'DELETE' });

export const markRoundSeen = (roundId: number) => api(`/rounds/${roundId}/seen`, { method: 'POST' });

export const retireRoundQuestion = (roundId: number) => api(`/rounds/${roundId}/question`, { method: 'DELETE' });

export const setRoundPinned = (roundId: number, pinned: boolean) => api(`/rounds/${roundId}/pin`, { method: pinned ? 'PUT' : 'DELETE' });
