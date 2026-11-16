/** First retry after a drop; each failed try doubles it, up to `MAX_RETRY_MS`. */
const RETRY_MS = 1000;
const MAX_RETRY_MS = 60_000;
/** The server hung up on purpose (seat or session gone, party over, unexpected message): no hurry to come back. */
const POLICY_RETRY_MS = 60_000;

/** Somewhere between half and all of `ms`: phones that lost the server together don't all come back at once. */
const jitter = (ms: number) => ms / 2 + Math.random() * (ms / 2);

/**
 * A WebSocket that reconnects by itself after it drops, until `close()`: 1 s, 2 s, 4 s… up to a minute, with jitter
 * (back to 1 s once a connection opens). Messages that aren't JSON are ignored.
 */
export function openSocket<T>(url: string | (() => Promise<string>), handlers: { onOpen?: () => void; onEvent: (event: T) => void }) {
  let ws: WebSocket | null = null;
  let stopped = false;
  let retry: ReturnType<typeof setTimeout>;
  let delay = RETRY_MS;
  const later = (ms = delay) => {
    retry = setTimeout(connect, jitter(ms));
    delay = Math.min(delay * 2, MAX_RETRY_MS);
  };
  const connect = async () => {
    // A URL that can't be had yet (a ticket from the server) is asked again at the next try.
    const href = typeof url === 'string' ? url : await url().catch(() => null);
    if (stopped) return;
    if (!href) {
      later();
      return;
    }
    ws = new WebSocket(href);
    ws.onopen = () => {
      delay = RETRY_MS;
      handlers.onOpen?.();
    };
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
      if (!stopped) later(policy ? Math.max(POLICY_RETRY_MS, delay) : delay);
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
