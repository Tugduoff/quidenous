// « Aujourd'hui »: every room of mine on one screen, what each one wants from me today (GET /api/today).
// Also when the app opens on it: with two rooms or more, at a plain cold start (see `useLandOnToday`).
import { useQuery } from '@tanstack/react-query';
import { useIsFocused } from 'expo-router';
import * as Linking from 'expo-linking';
import { useEffect, useState, useSyncExternalStore } from 'react';
import { Platform } from 'react-native';

import { api, plainStart, type QuestionKind, type Role } from './api';
import { useFeatures } from './auth';
import { openedFromNotification } from './native-push';
import { keys, useRooms } from './queries';
import { storage } from './storage';

/** What a room wants from me, most pressing first (the server orders the rooms by it). */
export type TodayState = 'vote' | 'result' | 'tripot' | 'wait' | 'idle';

export type TodayRoom = {
  id: number;
  name: string;
  role: Role;
  members: number;
  muted: boolean;
  state: TodayState;
  /** The open question, null when none. */
  round: { id: number; number: number; text: string; kind: QuestionKind; anonymous: boolean; opened_at: string } | null;
  voted: boolean;
  /** Votes in and votes needed to close (people skipped or away aren't waited for). */
  votes: number;
  needed: number;
  /** The newest result I voted on and haven't seen, `unseen` of them in all. */
  result: { id: number; number: number; text: string; kind: QuestionKind; closed_at: string } | null;
  unseen: number;
  /** Null when the room has no Tripot; `can_bet`: I can still bet on the open question. */
  tripot: { can_bet: boolean; balance: number } | null;
  /** My day streak in this room, and whether today already counts. */
  streak: { days: number; today: boolean } | null;
  unread: number;
  /** A paused room: when its next question opens by itself (null: the owner launches it). */
  next_open_at: string | null;
};

export type Today = { rooms: TodayRoom[] };

/**
 * Every room at once. `active`: only the focused tab listens, so events in other rooms don't refetch it behind
 * the vote screen; back on the tab it refetches if anything changed meanwhile (stale at once: each visit is fresh).
 */
export const useToday = (active: boolean) => {
  const { today } = useFeatures();
  return useQuery({ queryKey: keys.today, queryFn: () => api<Today>('/today'), enabled: today, subscribed: active, staleTime: 0 });
};

/** Something only I can do: vote, or see a result. */
export const needsMe = (r: TodayRoom) => r.state === 'vote' || r.state === 'result';

/** How many votes the question still waits for. */
export const missing = (r: TodayRoom) => Math.max(0, r.needed - r.votes);

/** My streak would break tonight: it counts days and today has no vote yet, while a question is open for me. */
export const streakAtRisk = (r: TodayRoom) => r.state === 'vote' && !!r.streak && r.streak.days > 0 && !r.streak.today;

/** The question a card shows: the result to see, else the open one. */
export const cardQuestion = (r: TodayRoom) => (r.state === 'result' ? r.result : r.round);

// ---------- the Today tab and landing on it ----------

/** '1': last known, the Today tab is there (feature on, 2+ rooms); '0': it isn't. Read before the first screen. */
const TAB_KEY = 'qdn.today';
/** The session's own key: signed in when the app started (not signing in now, which is navigating). */
const TOKEN_KEY = 'qdn.token';

let saved: string | null = null;
let signedInAtStart = false;
let loaded = false;
const listeners = new Set<() => void>();
Promise.all([storage.get(TAB_KEY), storage.get(TOKEN_KEY)])
  .then(([tab, token]) => {
    saved = tab;
    signedInAtStart = !!token;
  })
  .catch(() => {})
  .finally(() => {
    loaded = true;
    listeners.forEach((l) => l());
  });
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => void listeners.delete(l);
};

function remember(on: boolean) {
  const value = on ? '1' : '0';
  if (saved === value) return;
  saved = value;
  storage.set(TAB_KEY, value).catch(() => {});
  listeners.forEach((l) => l());
}

/**
 * Whether the Today tab exists: the server has it and I'm in two rooms or more (parties aside). With a single room
 * it would be a one-card detour before the vote. Until the server answers, the last known answer (no tab pops in or out
 * at start); every answer is remembered for the next start.
 */
export function useTodayTab(): boolean {
  const features = useFeatures();
  const { data: rooms } = useRooms();
  const last = useSyncExternalStore(subscribe, () => saved);
  const featuresKnown = features.query.data != null;
  const on = featuresKnown && features.today && rooms ? rooms.filter((r) => r.kind !== 'party').length >= 2 : null;
  const known = featuresKnown && (!features.today || rooms != null);
  useEffect(() => {
    if (known) remember(!!on);
  }, [known, on]);
  return known ? !!on : last === '1';
}

/**
 * The app was opened plainly (its icon, the bare web address), not by a notification, a `?room=` link or another link:
 * those open their own screen and keep it.
 */
function plainLaunch() {
  if (openedFromNotification()) return false;
  if (Platform.OS === 'web') return plainStart();
  const url = Linking.getLinkingURL();
  if (!url) return true;
  const { path, queryParams } = Linking.parse(url);
  return !path?.replace(/^\/+|\/+$/g, '') && !Object.keys(queryParams ?? {}).length;
}

/** The vote screen has shown once: from then on it's the user's (and every link's) to navigate. */
let landed = false;

/**
 * Called by the vote screen (the `/` route) on its first render: true when the app should open on Today instead,
 * i.e. at a plain cold start, signed in from the start, with the Today tab known from last time. The vote screen then
 * redirects straight away (it draws nothing meanwhile). Once and for all: never again after that first render.
 */
export function useLandOnToday() {
  const tab = useTodayTab();
  const focused = useIsFocused();
  const [land, setLand] = useState(() => !landed && loaded && signedInAtStart && saved === '1' && plainLaunch());
  useEffect(() => {
    landed = true;
  }, []);
  useEffect(() => {
    if (!land) return;
    // Left for Today (the vote screen is itself again behind it), or the tab went away meanwhile (no redirect possible);
    // else the redirect didn't happen (shouldn't): never leave the vote screen blank.
    const giveUp = setTimeout(() => setLand(false), !focused || !tab ? 0 : 500);
    return () => clearTimeout(giveUp);
  }, [land, focused, tab]);
  return land && focused && tab;
}
