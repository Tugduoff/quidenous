// Party mode (docs/PARTY.md): one question at a time for everyone in the room, a countdown, a big reveal.
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import { api, API_URL, ApiError, type Member, type Role, type Tally } from './api';
import { authError } from './auth';
import { t, type TKey } from './i18n';
import { localizeMembers } from './names';
import { appUrl } from './share';

export type PartyPhase = 'lobby' | 'voting' | 'reveal' | 'over';
export type PartyPlayer = Member & { role: Role; guest: boolean };
export type PartyRound = {
  id: number;
  number: number;
  question: string;
  opened_at: string;
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
  next: (id: number) => post(`/parties/${id}/next`),
  unnext: (id: number) => call(`/parties/${id}/next`, { method: 'DELETE' }),
  react: (id: number, round_id: number, emoji: string) => post(`/parties/${id}/react`, { round_id, emoji }),
  update: (id: number, body: { name?: string; timer?: number }) => call(`/parties/${id}`, { method: 'PATCH', json: body }),
  end: (id: number) => post(`/parties/${id}/end`),
  keep: (id: number) => api<{ id: number }>(`/parties/${id}/keep`, { method: 'POST' }),
  leave: (id: number) => api(`/rooms/${id}/leave`, { method: 'POST' }),
};

/** A party's state; the screen keeps it fresh from the live socket (lib/live.ts), with a slow poll in case it drops. */
export const useParty = (id: number) =>
  useQuery({ queryKey: partyKeys.party(id), queryFn: () => partyApi.get(id), refetchInterval: 20_000, retry: 1 });

/** Puts a fresh state from a mutation's answer straight in the cache. */
export function useSetParty(id: number) {
  const qc = useQueryClient();
  return (p: Party) => qc.setQueryData(partyKeys.party(id), p);
}

/** The link people scan or tap to join. */
export const partyJoinLink = (code: string) => `${appUrl()}/party/join/${code}`;
/** The big-screen view of a party (a TV, a laptop). */
export const partyScreenLink = (code: string) => `${appUrl()}/party/screen/${code}`;

/** Milliseconds left until `deadline` on the server's clock, ticking (0 once past, null without a deadline). */
export function useCountdown(deadline: string | null | undefined, clock: number) {
  const end = deadline ? Date.parse(deadline) : NaN;
  const left = () => (Number.isFinite(end) ? Math.max(0, end - (Date.now() + clock)) : null);
  const [ms, setMs] = useState(left);
  useEffect(() => {
    setMs(left());
    if (!Number.isFinite(end)) return;
    const timer = setInterval(() => {
      const v = left();
      setMs(v);
      if (v === 0) clearInterval(timer);
    }, 200);
    return () => clearInterval(timer);
    // `left` only reads these two.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [end, clock]);
  return ms;
}

/** Once the countdown hits 0: refetch a little later (and again every few seconds), in case the closing event got lost. */
export function useRefetchAtZero(ms: number | null, refetch: () => void) {
  const over = ms === 0;
  useEffect(() => {
    if (!over) return;
    let again: ReturnType<typeof setInterval>;
    const first = setTimeout(() => {
      refetch();
      again = setInterval(refetch, 4000);
    }, 1500);
    return () => {
      clearTimeout(first);
      clearInterval(again);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [over]);
}

/** French spacing kept on one line: no "?" or "!" alone at the start of a line. */
export const tidy = (text: string) => text.replace(/ ([?!:;»])/g, '\u00A0$1').replace(/« /g, '«\u00A0');

/** A result as the shared result bars draw it. */
export const tallies = (round: PartyRound): Tally[] =>
  (round.results ?? []).map((r) => ({ target_id: r.member_id, count: r.votes, voter_ids: r.voters })).sort((a, b) => b.count - a.count);

/** The big-screen view's live socket (no account: the party's code instead of a token). */
export function useScreenLive(code: string, onEvent: () => void) {
  useEffect(() => {
    if (!code) return;
    let ws: WebSocket | null = null;
    let stopped = false;
    let retry: ReturnType<typeof setTimeout>;
    const connect = () => {
      ws = new WebSocket(`${API_URL.replace(/^http/, 'ws')}/api/live?screen=${encodeURIComponent(code)}`);
      ws.onopen = onEvent;
      ws.onmessage = (msg) => {
        try {
          const event = JSON.parse(String(msg.data)) as { type: string };
          if (PARTY_EVENTS.has(event.type)) onEvent();
        } catch {}
      };
      ws.onclose = () => {
        if (!stopped) retry = setTimeout(connect, 3000);
      };
    };
    connect();
    return () => {
      stopped = true;
      clearTimeout(retry);
      ws?.close();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [code]);
}

const ERRORS: Record<string, TKey> = {
  name_taken: 'party.errors.nameTaken',
  party_full: 'party.errors.full',
  party_over: 'party.errors.over',
  not_found: 'party.errors.notFound',
  need_two: 'party.errors.needTwo',
  joined_late: 'party.vote.late',
  bad_name: 'party.errors.badName',
  bad_color: 'party.errors.badColor',
  not_party: 'party.errors.notParty',
  bad_room_name: 'room.errors.badName',
  bad_vibe: 'room.errors.badVibe',
};

/** A short message for a failed party call, in my language. */
export const partyError = (e: unknown) => (e instanceof ApiError && ERRORS[e.code] ? t(ERRORS[e.code]) : authError(e));
