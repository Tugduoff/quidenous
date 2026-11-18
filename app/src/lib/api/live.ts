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

const startsWith = (key: QueryKey, prefix: QueryKey) => prefix.every((k, i) => key[i] === k);

const HERE_MS = 20_000;
const BATCH_MS = 250;
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

export const useUnreachable = () => useSyncExternalStore(subscribe, () => unreachable, () => false);

type LiveEvent = { type: string; round_id?: number };

function touched(event: LiveEvent): QueryKey[] | 'all' {
  switch (event.type) {
    case 'party_updated':
      return [];
    case 'room_activity':
      return [todayKeys.all, roomKeys.all];
    case 'vote_cast':
    case 'skip_voted':
    case 'tripot_updated':
      return [roundKeys.current, todayKeys.all];
    case 'round_updated':
      return event.round_id ? [roundKeys.current, roundKeys.detail(event.round_id), todayKeys.all] : [roundKeys.current, todayKeys.all];
    case 'round_opened':
      return [roundKeys.all, historyKeys.list, questionKeys.queue, statsKeys.all, todayKeys.all];
    case 'round_closed':
      return [roundKeys.all, historyKeys.list, questionKeys.queue, statsKeys.all, badgeKeys.titles, todayKeys.all];
    case 'message_posted':
      return event.round_id ? [chatKeys.messages(event.round_id), todayKeys.all] : [todayKeys.all];
    case 'message_reacted':
      return event.round_id ? [chatKeys.messages(event.round_id)] : [];
    case 'message_deleted':
      return event.round_id ? [chatKeys.messages(event.round_id), inboxKeys.all, todayKeys.all] : [inboxKeys.all, todayKeys.all];
    case 'inbox_changed':
      return [inboxKeys.all, friendKeys.all, roomKeys.invites, badgeKeys.mine, badgeKeys.titles, todayKeys.all];
    case 'kick_voted':
      return [groupKeys.all];
    case 'round_pinned':
      return event.round_id ? [historyKeys.pinned, roundKeys.detail(event.round_id)] : [historyKeys.pinned];
    case 'question_added':
      return [questionKeys.queue];
    case 'room_updated':
      return [groupKeys.all, roomKeys.all, roundKeys.current, questionKeys.queue, todayKeys.all];
    case 'member_joined':
    case 'member_removed':
      return [groupKeys.all, roundKeys.current, roomKeys.all, roomKeys.invites, todayKeys.all];
    case 'member_updated':
      return [groupKeys.all, roundKeys.all, chatKeys.all];
    case 'room_deleted':
      return [groupKeys.all, roomKeys.all, roundKeys.current, todayKeys.all];
    case 'result_reacted':
      return [];
    default:
      return 'all';
  }
}

export function connectLive(qc: QueryClient, token: string): () => void {
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
      const features = await qc.ensureQueryData(featuresQuery);
      return liveUrl(token, features.includes('tickets'), features.includes('result_reactions'), features.includes('today'));
    },
    {
      onOpen: () => {
        reached();
        presence();
        qc.invalidateQueries({ predicate: (q) => !startsWith(q.queryKey, historyKeys.list) });
        refreshHistory(qc).catch(() => {});
      },
      onClose: dropped,
      onEvent: (event) => {
        if (event.type === 'result_reacted') {
          receiveReacted(qc, event as ReactedEvent);
          return;
        }
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

const liveUrl = async (t: string, tickets: boolean, reactions = false, others = false) =>
  `${API_URL.replace(/^http/, 'ws')}/api/live?${await urlAuth('live', tickets, t)}${roomQuery()}${reactions ? '&events=reactions' : ''}${others ? '&others=1' : ''}`;
