import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { AppState } from 'react-native';

import { liveUrl } from './api';
import { PARTY_EVENTS, partyKeys } from './party';
import { keys } from './queries';

const RETRY_MS = 3000;
/** While on the screen, the app tells the server every so often, so it sends no push for this room. */
const HERE_MS = 20_000;

/** Keeps every screen fresh: each server event invalidates the queries it touches. */
export function useLive(token: string | null, room: number | null) {
  const qc = useQueryClient();
  useEffect(() => {
    if (!token) return;
    let ws: WebSocket | null = null;
    let stopped = false;
    let retry: ReturnType<typeof setTimeout>;
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
        const event = JSON.parse(String(msg.data)) as { type: string; round_id?: number };
        // A party's screen follows every event of its room (the socket only carries the current room's).
        if (PARTY_EVENTS.has(event.type)) qc.invalidateQueries({ queryKey: partyKeys.all });
        switch (event.type) {
          case 'party_updated':
            break;
          case 'vote_cast':
            qc.invalidateQueries({ queryKey: keys.current });
            break;
          case 'round_closed':
          case 'round_opened':
            qc.invalidateQueries({ queryKey: ['round'] });
            qc.invalidateQueries({ queryKey: keys.history });
            qc.invalidateQueries({ queryKey: keys.queue });
            qc.invalidateQueries({ queryKey: keys.stats });
            break;
          case 'message_posted':
            if (event.round_id) qc.invalidateQueries({ queryKey: keys.messages(event.round_id) });
            break;
          case 'inbox_changed':
            qc.invalidateQueries({ queryKey: keys.inbox });
            break;
          case 'skip_voted':
            qc.invalidateQueries({ queryKey: keys.current });
            break;
          case 'kick_voted':
            qc.invalidateQueries({ queryKey: keys.group });
            break;
          case 'round_pinned':
            qc.invalidateQueries({ queryKey: keys.history });
            if (event.round_id) qc.invalidateQueries({ queryKey: keys.round(event.round_id) });
            break;
          case 'question_added':
            qc.invalidateQueries({ queryKey: keys.queue });
            break;
          default:
            qc.invalidateQueries();
        }
      };
      ws.onclose = () => {
        if (!stopped) retry = setTimeout(connect, RETRY_MS);
      };
    };
    connect();
    return () => {
      stopped = true;
      clearTimeout(retry);
      clearInterval(heartbeat);
      appState.remove();
      ws?.close();
    };
    }, [token, room, qc]);
}
