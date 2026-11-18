import type { QueryClient, QueryKey } from '@tanstack/react-query';
import { useSyncExternalStore } from 'react';
import { AppState } from 'react-native';
import { API_URL, urlAuth, roomQuery } from '@/lib/api/client';
import { badgeKeys } from '@/lib/api/badges';
import { PARTY_EVENTS, partyKeys } from '@/lib/api/party';
import { type ReactedEvent, receiveReacted } from '@/lib/api/result-reactions';
import { openSocket } from '@/lib/api/socket';
import { refreshHistory, historyKeys } from '@/lib/api/history';
import { roundKeys } from '@/lib/api/rounds';
import { friendKeys } from '@/lib/api/friends';
import { groupKeys } from '@/lib/api/group';
import { chatKeys } from '@/lib/api/chat';
import { todayKeys } from '@/lib/api/today';
import { statsKeys } from '@/lib/api/stats';
import { questionKeys } from '@/lib/api/questions';
import { inboxKeys } from '@/lib/api/inbox';
import { roomKeys } from '@/lib/api/rooms';
import { featuresQuery } from '@/lib/api/features';

/** While on the screen, the app tells the server every so often, so it sends no push for this room. */
const startsWith = (key: QueryKey, prefix: QueryKey) => prefix.every((k, i) => key[i] === k);

const HERE_MS = 20_000;
/** Events in a burst (a round closing, guests joining a party) refetch each query once. */
const BATCH_MS = 250;
/** The live connection has been down this long (not on purpose): the server counts as unreachable. */
const DOWN_MS = 10_000;

let unreachable = false;
const listeners = new Set<() => void>();
function setUnreachable(value: boolean) {
  if (value === unreachable) return;
  unreachable = value;
  for (const l of listeners) l();
}
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => void listeners.delete(l);
};

/** Signed in and the server can't be reached (the live connection keeps failing): the screens say so. */
export const useUnreachable = () => useSyncExternalStore(subscribe, () => unreachable, () => false);

type LiveEvent = { type: string; round_id?: number };

/**
 * The queries each server event touches (server/src/live.rs). Unknown events: everything, like `resync`.
 * `todayKeys.all` (all my rooms) follows what changes a room's card: votes, questions, chat, bets, who's in.
 */
function touched(event: LiveEvent): QueryKey[] | 'all' {
  switch (event.type) {
    case 'party_updated':
      return [];
    // Something happened in another room of mine (`others=1`): only « Aujourd'hui » and the room list show it.
    case 'room_activity':
      return [todayKeys.all, roomKeys.all];
    case 'vote_cast':
    case 'skip_voted':
    // Someone bet on the open question (Tripot): its pot changed.
    case 'tripot_updated':
      return [roundKeys.current, todayKeys.all];
    // A « Qui a dit ça ? » question went from writing to guessing (docs/SAID.md).
    case 'round_updated':
      return event.round_id ? [roundKeys.current, roundKeys.detail(event.round_id), todayKeys.all] : [roundKeys.current, todayKeys.all];
    case 'round_opened':
      return [roundKeys.all, historyKeys.list, questionKeys.queue, statsKeys.all, todayKeys.all];
    // A closed question can cross a badge threshold (mine arrives as an inbox entry; the room's titles may follow).
    case 'round_closed':
      return [roundKeys.all, historyKeys.list, questionKeys.queue, statsKeys.all, badgeKeys.titles, todayKeys.all];
    case 'message_posted':
      return event.round_id ? [chatKeys.messages(event.round_id), todayKeys.all] : [todayKeys.all];
    // Someone reacted to a message (or took their reaction back).
    case 'message_reacted':
      return event.round_id ? [chatKeys.messages(event.round_id)] : [];
    // Deleted by its author, the owner or an admin, or hidden by reports.
    case 'message_deleted':
      return event.round_id ? [chatKeys.messages(event.round_id), inboxKeys.all, todayKeys.all] : [inboxKeys.all, todayKeys.all];
    // Also a friend request, a room invitation or a new badge for me.
    case 'inbox_changed':
      return [inboxKeys.all, friendKeys.all, roomKeys.invites, badgeKeys.mine, badgeKeys.titles, todayKeys.all];
    case 'kick_voted':
      return [groupKeys.all];
    case 'round_pinned':
      return event.round_id ? [historyKeys.pinned, roundKeys.detail(event.round_id)] : [historyKeys.pinned];
    case 'question_added':
      return [questionKeys.queue];
    // Settings, owner, pacing, invite code: the room and its open question (skip rules, next opening).
    case 'room_updated':
      return [groupKeys.all, roomKeys.all, roundKeys.current, questionKeys.queue, todayKeys.all];
    // Who's in: the room, the counts on the open question and in the room list.
    case 'member_joined':
    case 'member_removed':
      return [groupKeys.all, roundKeys.current, roomKeys.all, roomKeys.invites, todayKeys.all];
    // A new name or photo: also shown on results and in the chat.
    case 'member_updated':
      return [groupKeys.all, roundKeys.all, chatKeys.all];
    case 'room_deleted':
      return [groupKeys.all, roomKeys.all, roundKeys.current, todayKeys.all];
    // Taps on a result: handed to its screen (receiveReacted), never a refetch (they come in bursts).
    case 'result_reacted':
      return [];
    default:
      return 'all';
  }
}

/** Keeps every screen fresh: each server event invalidates the queries it touches. Returns the disconnect. */
export function connectLive(qc: QueryClient, token: string): () => void {
  // Queries to refetch at the end of the current burst ('all': everything).
  let batch = new Map<string, QueryKey>();
  let batchAll = false;
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const flush = () => {
    flushTimer = null;
    if (batchAll) {
      qc.invalidateQueries({ predicate: (q) => !startsWith(q.queryKey, historyKeys.list) });
      refreshHistory(qc).catch(() => {});
    } else
      for (const queryKey of batch.values()) {
        // The history list gets its newest page only (see refreshHistory).
        if (queryKey === historyKeys.list) refreshHistory(qc).catch(() => {});
        else qc.invalidateQueries({ queryKey });
      }
    batch = new Map();
    batchAll = false;
  };
  const invalidate = (queryKeys: QueryKey[] | 'all') => {
    if (queryKeys === 'all') batchAll = true;
    else for (const k of queryKeys) batch.set(JSON.stringify(k), k);
    if (!flushTimer && (batchAll || batch.size)) flushTimer = setTimeout(flush, BATCH_MS);
  };
  // Dropped (not on purpose) and still down after DOWN_MS: unreachable, until it opens again.
  let down: ReturnType<typeof setTimeout> | null = null;
  const dropped = (policy: boolean) => {
    if (!policy && !down) down = setTimeout(() => setUnreachable(true), DOWN_MS);
  };
  const reached = () => {
    if (down) clearTimeout(down);
    down = null;
    setUnreachable(false);
  };
  const presence = () => {
    socket.send(AppState.currentState === 'active' ? 'here' : 'away');
  };
  const heartbeat = setInterval(presence, HERE_MS);
  const appState = AppState.addEventListener('change', (state) => {
    presence();
    // Back in the app: reconnect now (not at the end of a backoff), and give it DOWN_MS before saying anything.
    if (state === 'active') {
      if (down || unreachable) {
        reached();
        dropped(false);
      }
      socket.wake();
    }
  });
  const socket = openSocket<LiveEvent>(
    async () => {
      // A short-lived ticket instead of the token in the URL, when the server has them. Features not known yet
      // (cold start): fetched first; unreachable: no socket this time (retried), never the token by default.
      // With « Aujourd'hui », my other rooms' activity too.
      const features = await qc.ensureQueryData(featuresQuery);
      // `result_reacted` only for an app that knows it (an older one would refetch everything on each).
      return liveUrl(token, features.includes('tickets'), features.includes('result_reactions'), features.includes('today'));
    },
    {
      onOpen: () => {
        reached();
        presence();
        // Everything but the history list's pages (one refetch each): only its newest page.
        qc.invalidateQueries({ predicate: (q) => !startsWith(q.queryKey, historyKeys.list) });
        refreshHistory(qc).catch(() => {});
      },
      onClose: dropped,
      onEvent: (event) => {
        if (event.type === 'result_reacted') {
          receiveReacted(qc, event as ReactedEvent);
          return;
        }
        // A party's screen follows every event of its room (the socket only carries the current room's).
        if (PARTY_EVENTS.has(event.type)) invalidate([partyKeys.all]);
        invalidate(touched(event));
      },
    },
  );
  return () => {
    if (flushTimer) clearTimeout(flushTimer);
    if (down) clearTimeout(down);
    setUnreachable(false);
    clearInterval(heartbeat);
    appState.remove();
    socket.close();
  };
}

/**
 * The live socket. `reactions`: also send `result_reacted` (only to apps that handle it, docs/REACTIONS.md).
 * `others`: also a `room_activity` event for my other rooms (the server has `today`, docs/TODAY.md).
 */
export const liveUrl = async (t: string, tickets: boolean, reactions = false, others = false) =>
  `${API_URL.replace(/^http/, 'ws')}/api/live?${await urlAuth('live', tickets, t)}${roomQuery()}${reactions ? '&events=reactions' : ''}${others ? '&others=1' : ''}`;
