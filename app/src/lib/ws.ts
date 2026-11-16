const RETRY_MS = 3000;
/** The server hung up on purpose (seat or session gone, party over, unexpected message): no hurry to come back. */
const POLICY_RETRY_MS = 60_000;

/** A WebSocket that reconnects by itself after it drops, until `close()`. Messages that aren't JSON are ignored. */
export function openSocket<T>(url: string | (() => Promise<string>), handlers: { onOpen?: () => void; onEvent: (event: T) => void }) {
  let ws: WebSocket | null = null;
  let stopped = false;
  let retry: ReturnType<typeof setTimeout>;
  const connect = async () => {
    // A URL that can't be had yet (a ticket from the server) is asked again at the next try.
    const href = typeof url === 'string' ? url : await url().catch(() => null);
    if (stopped) return;
    if (!href) {
      retry = setTimeout(connect, RETRY_MS);
      return;
    }
    ws = new WebSocket(href);
    ws.onopen = () => handlers.onOpen?.();
    ws.onmessage = (msg) => {
      let event: T;
      try {
        event = JSON.parse(String(msg.data)) as T;
      } catch {
        return;
      }
      handlers.onEvent(event);
    };
    ws.onclose = (e) => {
      const policy = e.code === 1008 && e.reason !== 'idle';
      if (!stopped) retry = setTimeout(connect, policy ? POLICY_RETRY_MS : RETRY_MS);
    };
  };
  connect();
  return {
    send: (text: string) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(text);
    },
    close: () => {
      stopped = true;
      clearTimeout(retry);
      ws?.close();
    },
  };
}
