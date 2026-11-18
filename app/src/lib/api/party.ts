// Party mode (docs/PARTY.md): one question at a time for everyone in the room, a countdown, a big reveal.
import { type QueryClient, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, API_URL, ApiError } from '@/lib/api/client';
import { t, type TKey } from '@/lib/i18n';
import { localizeMembers } from '@/lib/utils/names';
import { type ReactedEvent, receiveReacted } from '@/lib/api/result-reactions';
import { openSocket } from '@/lib/api/socket';
import { type Member, type Role, leaveRoom } from '@/lib/api/group';
import type { QuestionKind, SaidGuess, SaidView } from '@/lib/api/rounds';
import { getHistoryPage, HISTORY_PAGE, type HistoryItem } from '@/lib/api/history';
import { authError } from '@/lib/api/errors';
import { featuresQuery } from '@/lib/api/features';

export type PartyPhase = 'lobby' | 'voting' | 'reveal' | 'over';
export type PartyPlayer = Member & { role: Role; guest: boolean };
export type PartyRound = {
  id: number;
  number: number;
  question: string;
  /** `pick` or `said` (absent from an older server: a pick). */
  kind?: QuestionKind;
  /** A said question (docs/SAID.md): its phase, answers and result; `my_*` null on the big screen. */
  said?: SaidView | null;
  opened_at: string;
  /** The current phase's deadline (a said question: the writing's, then the guessing's). */
  deadline_at: string | null;
  closed_at: string | null;
  /** Who was there when it opened: only they vote, only they can be picked. */
  eligible: number[];
  /** Who voted (not for whom). */
  voted: number[];
  my_vote: number | null;
  /** null while voting: votes are revealed together. */
  results: { member_id: number; votes: number; voters: number[] }[] | null;
  next: { taps: number[]; needed: number; players: number; mine: boolean } | null;
  reactions: { emoji: string; count: number; mine: boolean }[] | null;
};
export type Party = {
  id: number;
  kind: 'party' | 'room';
  name: string;
  code: string;
  vibe: string | null;
  language: 'fr' | 'en';
  timer: number;
  ends_at: string;
  /** The owner closed the door: nobody new can join (absent from an older server). */
  locked?: boolean;
  now: string;
  phase: PartyPhase;
  players: PartyPlayer[];
  /** null on the big screen (no account). */
  me: { member_id: number; role: Role; guest: boolean } | null;
  round: PartyRound | null;
  played: number;
  /** Added here: server clock minus mine, in ms, so the countdown matches everyone's. */
  clock: number;
};

export const HOURS = [2, 4, 8, 12] as const;
export const TIMERS = [15, 20, 30, 45, 60] as const;
/** The quick reactions on a result (content, so emojis are fine here). */
export const REACTIONS = ['😂', '🔥', '😱', '👀', '💀', '❤️'] as const;

/** Live events that change a party's state: its screen refetches on each. */
export const PARTY_EVENTS = new Set([
  'member_joined',
  'vote_cast',
  // A said question's guessing started (docs/SAID.md).
  'round_updated',
  'round_closed',
  'round_opened',
  'member_removed',
  'member_updated',
  'room_updated',
  'party_updated',
  'resync',
]);

export const partyKeys = {
  all: ['party'] as const,
  party: (id: number) => ['party', id] as const,
  screen: (code: string) => ['party', 'screen', code] as const,
  memory: (id: number) => ['party', id, 'memory'] as const,
};

/** Players named as the app shows them, and the server's clock offset measured on arrival. */
const received = (p: Omit<Party, 'clock'>): Party => ({
  ...p,
  players: localizeMembers(p.players ?? []),
  clock: Number.isFinite(Date.parse(p.now)) ? Date.parse(p.now) - Date.now() : 0,
});

const call = (path: string, init: Parameters<typeof api>[1] = {}) => api<Omit<Party, 'clock'>>(path, init).then(received);
const post = (path: string, json?: unknown) => call(path, { method: 'POST', json });

export const partyApi = {
  create: (body: { name?: string; vibe?: string; language?: string; hours: number; timer: number }) =>
    api<{ id: number; code: string }>('/parties', { method: 'POST', json: body }),
  join: (body: { code: string; name?: string; color?: string }) =>
    api<{ party_id: number; member_id: number; token?: string }>('/parties/join', { method: 'POST', json: body }),
  get: (id: number) => call(`/parties/${id}`),
  screen: (code: string) => call(`/parties/screen/${encodeURIComponent(code)}`),
  start: (id: number) => post(`/parties/${id}/start`),
  vote: (id: number, target_id: number) => post(`/parties/${id}/vote`, { target_id }),
  /** A said question: write (or rewrite) my answer, until the writing deadline. */
  answer: (id: number, text: string) => post(`/parties/${id}/answer`, { text }),
  /** A said question: who wrote what, one guess per answer that isn't mine. */
  guess: (id: number, guesses: SaidGuess[]) => post(`/parties/${id}/vote`, { guesses }),
  next: (id: number) => post(`/parties/${id}/next`),
  unnext: (id: number) => call(`/parties/${id}/next`, { method: 'DELETE' }),
  react: (id: number, round_id: number, emoji: string) => post(`/parties/${id}/react`, { round_id, emoji }),
  update: (id: number, body: { name?: string; timer?: number; locked?: boolean }) => call(`/parties/${id}`, { method: 'PATCH', json: body }),
  end: (id: number) => post(`/parties/${id}/end`),
  keep: (id: number) => api<{ id: number }>(`/parties/${id}/keep`, { method: 'POST' }),
  leave: leaveRoom,
};

/** A party's state; the screen keeps it fresh from the live socket (lib/live.ts), with a slow poll in case it drops. */
export const useParty = (id: number) =>
  useQuery({ queryKey: partyKeys.party(id), queryFn: () => partyApi.get(id), refetchInterval: 20_000, retry: 1 });

/** Puts a fresh state from a mutation's answer straight in the cache. */
export function useSetParty(id: number) {
  const qc = useQueryClient();
  return (p: Party) => qc.setQueryData(partyKeys.party(id), p);
}

/**
 * The big-screen view's live socket (no account: the party's code instead of a token). Taps on a result
 * (`result_reacted`, asked for when the server has them) go to its bursts, the counts come with `party_updated`.
 */
export function connectScreen(qc: QueryClient, code: string, onChange: () => void): () => void {
  const url = async () => {
    const features = await qc.ensureQueryData(featuresQuery).catch(() => [] as string[]);
    const events = features.includes('result_reactions') ? '&events=reactions' : '';
    return `${API_URL.replace(/^http/, 'ws')}/api/live?screen=${encodeURIComponent(code)}${events}`;
  };
  const socket = openSocket<{ type: string }>(url, {
    onOpen: onChange,
    onEvent: (event) => {
      if (event.type === 'result_reacted') receiveReacted(qc, event as ReactedEvent);
      else if (PARTY_EVENTS.has(event.type)) onChange();
    },
  });
  return socket.close;
}

const ERRORS: Record<string, TKey> = {
  name_taken: 'party.errors.nameTaken',
  party_full: 'party.errors.full',
  party_over: 'party.errors.over',
  not_found: 'party.errors.notFound',
  need_two: 'party.errors.needTwo',
  joined_late: 'party.vote.late',
  time_up: 'party.vote.timeUp',
  not_writing: 'said.write.tooLate',
  not_guessing: 'said.party.notGuessing',
  bad_answer: 'said.write.failed',
  bad_guesses: 'said.guess.failed',
  bad_name: 'party.errors.badName',
  bad_color: 'party.errors.badColor',
  not_party: 'party.errors.notParty',
  party_locked: 'party.errors.locked',
  removed: 'party.errors.removed',
  bad_room_name: 'room.errors.badName',
  bad_vibe: 'room.errors.badVibe',
};

/** A short message for a failed party call, in my language. */
export const partyError = (e: unknown) => (e instanceof ApiError && ERRORS[e.code] ? t(ERRORS[e.code]) : authError(e));

const MEMORY_MAX_PAGES = 20;

export function usePartyMemory(id: number) {
  return useQuery({
    queryKey: partyKeys.memory(id),
    queryFn: async () => {
      const all: HistoryItem[] = [];
      let before: number | null = null;
      for (let i = 0; i < MEMORY_MAX_PAGES; i++) {
        const page = await getHistoryPage(before, id);
        all.push(...page);
        const last = page.at(-1);
        if (page.length < HISTORY_PAGE || !last) break;
        before = last.number;
      }
      return all;
    },
  });
}
