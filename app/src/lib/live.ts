import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { liveUrl } from './api';
import { keys } from './queries';

const RETRY_MS = 3000;

/** Keeps every screen fresh: each server event invalidates the queries it touches. */
export function useLive(token: string | null) {
  const qc = useQueryClient();
  useEffect(() => {
    if (!token) return;
    let ws: WebSocket | null = null;
    let stopped = false;
    let retry: ReturnType<typeof setTimeout>;
    const connect = () => {
      ws = new WebSocket(liveUrl(token));
      ws.onopen = () => qc.invalidateQueries();
      ws.onmessage = (msg) => {
        const event = JSON.parse(String(msg.data)) as { type: string; round_id?: number };
        switch (event.type) {
          case 'vote_cast':
            qc.invalidateQueries({ queryKey: keys.current });
            break;
          case 'round_closed':
          case 'round_opened':
            qc.invalidateQueries({ queryKey: ['round'] });
            qc.invalidateQueries({ queryKey: keys.history });
            qc.invalidateQueries({ queryKey: keys.queue });
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
      ws?.close();
    };
  }, [token, qc]);
}
