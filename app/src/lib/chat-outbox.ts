import { useSyncExternalStore } from 'react';
import { showToast } from '@/lib/toast';
import { ApiError, mediaUrl } from '@/lib/api/client';
import { queryClient } from '@/lib/api/query-client';
import { type Message, chatKeys, type Ping, uploadImage, type Gif, postGif } from '@/lib/api/chat';
import type { ImageDraft } from '@/lib/device/media';
import { authError } from '@/lib/api/errors';

/** A photo or GIF on its way: drawn at the end of the chat (dimmed, a spinner) until the server has it. */
export type Pending = {
  key: string;
  kind: 'image' | 'gif';
  /** What to draw meanwhile: the local file, or the GIF's preview. */
  uri: string;
  width: number;
  height: number;
  caption: string;
  /** The upload failed (offline, server down): a tap sends it again. */
  failed: boolean;
};

const EMPTY: Pending[] = [];
const pendingByRound = new Map<number, Pending[]>();
const senders = new Map<string, () => Promise<Message>>();
/** The server's idempotency key of each send: a retry whose first answer was lost is posted once. */
const newClientKey = () => `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
const listeners = new Set<() => void>();
let seq = 0;

const update = (roundId: number, f: (list: Pending[]) => Pending[]) => {
  const next = f(pendingByRound.get(roundId) ?? EMPTY);
  if (next.length) pendingByRound.set(roundId, next);
  else pendingByRound.delete(roundId);
  listeners.forEach((l) => l());
};

/** The photos and GIFs of this round still being sent. */
export function usePending(roundId: number) {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => pendingByRound.get(roundId) ?? EMPTY,
  );
}

/** Network trouble (worth a retry) rather than a refusal (bad image, too large, quota, closed round). */
const retryable = (e: unknown) => !(e instanceof ApiError) || e.status >= 500;

async function run(roundId: number, key: string, onSent?: () => void) {
  const send = senders.get(key);
  if (!send) return;
  update(roundId, (list) => list.map((p) => (p.key === key ? { ...p, failed: false } : p)));
  try {
    const m = await send();
    senders.delete(key);
    update(roundId, (list) => list.filter((p) => p.key !== key));
    queryClient.setQueryData<Message[]>(chatKeys.messages(roundId), (old) => (old?.some((x) => x.id === m.id) ? old : [...(old ?? []), m]));
    onSent?.();
  } catch (e) {
    if (retryable(e)) {
      update(roundId, (list) => list.map((p) => (p.key === key ? { ...p, failed: true } : p)));
      return;
    }
    senders.delete(key);
    update(roundId, (list) => list.filter((p) => p.key !== key));
    showToast(authError(e));
  }
}

function start(roundId: number, p: Omit<Pending, 'key' | 'failed'>, send: (clientKey: string) => Promise<Message>, onSent?: () => void) {
  const key = `p${++seq}`;
  const clientKey = newClientKey();
  senders.set(key, () => send(clientKey));
  update(roundId, (list) => [...list, { ...p, key, failed: false }]);
  void run(roundId, key, onSent);
}

export const sendImage = (roundId: number, img: ImageDraft, caption: string, ping: Ping, onSent?: () => void) =>
  start(
    roundId,
    { kind: img.mime === 'image/gif' ? 'gif' : 'image', uri: img.uri, width: img.width, height: img.height, caption },
    (clientKey) => uploadImage(roundId, img, caption, ping, clientKey),
    onSent,
  );

export const sendGif = (roundId: number, g: Gif, onSent?: () => void) =>
  start(
    roundId,
    { kind: 'gif', uri: mediaUrl(g.preview_url), width: g.width, height: g.height, caption: '' },
    (clientKey) => postGif(roundId, g, { mentions: [], everyone: false }, clientKey),
    onSent,
  );

export const retryPending = (roundId: number, key: string) => run(roundId, key);

export const discardPending = (roundId: number, key: string) => {
  senders.delete(key);
  update(roundId, (list) => list.filter((p) => p.key !== key));
};
