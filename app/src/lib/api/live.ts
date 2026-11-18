import { type QueryKey, useQueryClient } from '@tanstack/react-query';
import { useEffect, useSyncExternalStore } from 'react';
import { AppState } from 'react-native';

import { liveUrl } from '@/lib/api/client';
import { featuresQuery } from '@/lib/api/auth';
import { badgesKey, titlesKey } from '@/lib/api/badges';
import { PARTY_EVENTS, partyKeys } from '@/lib/api/party';
import { keys, refreshHistory } from '@/lib/queries';
import { type ReactedEvent, receiveReacted } from '@/lib/api/result-reactions';
import { openSocket } from '@/lib/api/socket';

/** While on the screen, the app tells the server every so often, so it sends no push for this room. */
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
 * `keys.today` (all my rooms) follows what changes a room's card: votes, questions, chat, bets, who's in.
 */
function touched(event: LiveEvent): QueryKey[] | 'all' {
  switch (event.type) {
    case 'party_updated':
      return [];
    // Something happened in another room of mine (`others=1`): only « Aujourd'hui » and the room list show it.
    case 'room_activity':
      return [keys.today, keys.rooms];
    case 'vote_cast':
    case 'skip_voted':
    // Someone bet on the open question (Tripot): its pot changed.
    case 'tripot_updated':
      return [keys.current, keys.today];
    // A « Qui a dit ça ? » question went from writing to guessing (docs/SAID.md).
    case 'round_updated':
      return event.round_id ? [keys.current, keys.round(event.round_id), keys.today] : [keys.current, keys.today];
    case 'round_opened':
      return [['round'], keys.historyList, keys.queue, keys.stats, keys.today];
    // A closed question can cross a badge threshold (mine arrives as an inbox entry; the room's titles may follow).
    case 'round_closed':
      return [['round'], keys.historyList, keys.queue, keys.stats, titlesKey, keys.today];
    case 'message_posted':
      return event.round_id ? [keys.messages(event.round_id), keys.today] : [keys.today];
    // Someone reacted to a message (or took their reaction back).
    case 'message_reacted':
      return event.round_id ? [keys.messages(event.round_id)] : [];
    // Deleted by its author, the owner or an admin, or hidden by reports.
    case 'message_deleted':
      return event.round_id ? [keys.messages(event.round_id), keys.inbox, keys.today] : [keys.inbox, keys.today];
    // Also a friend request, a room invitation or a new badge for me.
    case 'inbox_changed':
      return [keys.inbox, keys.friends, keys.invites, badgesKey, titlesKey, keys.today];
    case 'kick_voted':
      return [keys.group];
    case 'round_pinned':
      return event.round_id ? [keys.pinned, keys.round(event.round_id)] : [keys.pinned];
    case 'question_added':
      return [keys.queue];
    // Settings, owner, pacing, invite code: the room and its open question (skip rules, next opening).
    case 'room_updated':
      return [keys.group, keys.rooms, keys.current, keys.queue, keys.today];
    // Who's in: the room, the counts on the open question and in the room list.
    case 'member_joined':
    case 'member_removed':
      return [keys.group, keys.current, keys.rooms, ['invites'], keys.today];
    // A new name or photo: also shown on results and in the chat.
    case 'member_updated':
      return [keys.group, ['round'], ['messages']];
    case 'room_deleted':
      return [keys.group, keys.rooms, keys.current, keys.today];
    // Taps on a result: handed to its screen (receiveReacted), never a refetch (they come in bursts).
    case 'result_reacted':
      return [];
    default:
      return 'all';
  }
}

/** Keeps every screen fresh: each server event invalidates the queries it touches. */
export function useLive(token: string | null, room: number | null) {
  const qc = useQueryClient();
  useEffect(() => {
    if (!token) return;
    // Queries to refetch at the end of the current burst ('all': everything).
    let batch = new Map<string, QueryKey>();
    let batchAll = false;
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const flush = () => {
      flushTimer = null;
      if (batchAll) {
        qc.invalidateQueries({ predicate: (q) => q.queryKey[0] !== 'history' || q.queryKey[1] !== 'list' });
        refreshHistory(qc).catch(() => {});
      } else
        for (const queryKey of batch.values()) {
          // The history list gets its newest page only (see refreshHistory).
          if (queryKey === keys.historyList) refreshHistory(qc).catch(() => {});
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
          qc.invalidateQueries({ predicate: (q) => q.queryKey[0] !== 'history' || q.queryKey[1] !== 'list' });
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
  }, [token, room, qc]);
}
