import { type QueryKey, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { AppState } from 'react-native';

import { liveUrl } from './api';
import { PARTY_EVENTS, partyKeys } from './party';
import { keys } from './queries';

const RETRY_MS = 3000;
/** While on the screen, the app tells the server every so often, so it sends no push for this room. */
const HERE_MS = 20_000;
/** Events in a burst (a round closing, guests joining a party) refetch each query once. */
const BATCH_MS = 250;

type LiveEvent = { type: string; round_id?: number };

/** The queries each server event touches (server/src/live.rs). Unknown events: everything, like `resync`. */
function touched(event: LiveEvent): QueryKey[] | 'all' {
  switch (event.type) {
    case 'party_updated':
      return [];
    case 'vote_cast':
    case 'skip_voted':
      return [keys.current];
    case 'round_closed':
    case 'round_opened':
      return [['round'], keys.history, keys.queue, keys.stats];
    case 'message_posted':
      return event.round_id ? [keys.messages(event.round_id)] : [];
    case 'inbox_changed':
      return [keys.inbox];
    case 'kick_voted':
      return [keys.group];
    case 'round_pinned':
      return event.round_id ? [keys.history, keys.round(event.round_id)] : [keys.history];
    case 'question_added':
      return [keys.queue];
    // Settings, owner, pacing, invite code: the room and its open question (skip rules, next opening).
    case 'room_updated':
      return [keys.group, keys.rooms, keys.current, keys.queue];
    // Who's in: the room, the counts on the open question and in the room list.
    case 'member_joined':
    case 'member_removed':
      return [keys.group, keys.current, keys.rooms];
    // A new name or photo: also shown on results and in the chat.
    case 'member_updated':
      return [keys.group, ['round'], ['messages']];
    case 'room_deleted':
      return [keys.group, keys.rooms, keys.current];
    default:
      return 'all';
  }
}

/** Keeps every screen fresh: each server event invalidates the queries it touches. */
export function useLive(token: string | null, room: number | null) {
  const qc = useQueryClient();
  useEffect(() => {
    if (!token) return;
    let ws: WebSocket | null = null;
    let stopped = false;
    let retry: ReturnType<typeof setTimeout>;
    // Queries to refetch at the end of the current burst ('all': everything).
    let batch = new Map<string, QueryKey>();
    let batchAll = false;
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const flush = () => {
      flushTimer = null;
      if (batchAll) qc.invalidateQueries();
      else for (const queryKey of batch.values()) qc.invalidateQueries({ queryKey });
      batch = new Map();
      batchAll = false;
    };
    const invalidate = (queryKeys: QueryKey[] | 'all') => {
      if (queryKeys === 'all') batchAll = true;
      else for (const k of queryKeys) batch.set(JSON.stringify(k), k);
      if (!flushTimer && (batchAll || batch.size)) flushTimer = setTimeout(flush, BATCH_MS);
    };
    const presence = () => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(AppState.currentState === 'active' ? 'here' : 'away');
    };
    const heartbeat = setInterval(presence, HERE_MS);
    const appState = AppState.addEventListener('change', presence);
    const connect = () => {
      ws = new WebSocket(liveUrl(token));
      ws.onopen = () => {
        presence();
        qc.invalidateQueries();
      };
      ws.onmessage = (msg) => {
        const event = JSON.parse(String(msg.data)) as LiveEvent;
        // A party's screen follows every event of its room (the socket only carries the current room's).
        if (PARTY_EVENTS.has(event.type)) invalidate([partyKeys.all]);
        invalidate(touched(event));
      };
      ws.onclose = () => {
        if (!stopped) retry = setTimeout(connect, RETRY_MS);
      };
    };
    connect();
    return () => {
      stopped = true;
      clearTimeout(retry);
      if (flushTimer) clearTimeout(flushTimer);
      clearInterval(heartbeat);
      appState.remove();
      ws?.close();
    };
  }, [token, room, qc]);
}
