const RETRY_MS = 1000;
const MAX_RETRY_MS = 60_000;
const POLICY_RETRY_MS = 60_000;

const jitter = (ms: number) => ms / 2 + Math.random() * (ms / 2);

export function openSocket<T>(
  url: string | (() => Promise<string>),
  handlers: { onOpen?: () => void; onClose?: (policy: boolean) => void; onEvent: (event: T) => void },
) {
  let ws: WebSocket | null = null;
  let stopped = false;
  let retry: ReturnType<typeof setTimeout>;
  let delay = RETRY_MS;
  let fetching = false;
  const later = (ms = delay) => {
    retry = setTimeout(connect, jitter(ms));
    delay = Math.min(delay * 2, MAX_RETRY_MS);
  };
  const connect = async () => {
    fetching = true;
    const href = typeof url === 'string' ? url : await url().catch(() => null);
    fetching = false;
    if (stopped) return;
    if (!href) {
      handlers.onClose?.(false);
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
      if (stopped) return;
      handlers.onClose?.(policy);
      later(policy ? Math.max(POLICY_RETRY_MS, delay) : delay);
    };
  };
  connect();
  return {
    send: (text: string) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(text);
    },
    wake: () => {
      if (stopped || fetching || ws?.readyState === WebSocket.OPEN || ws?.readyState === WebSocket.CONNECTING) return;
      clearTimeout(retry);
      delay = RETRY_MS;
      connect();
    },
    close: () => {
      stopped = true;
      clearTimeout(retry);
      ws?.close();
    },
  };
}
