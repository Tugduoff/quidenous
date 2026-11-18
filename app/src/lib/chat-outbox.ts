import { useInfiniteQuery } from '@tanstack/react-query';
import { Directory, File as FsFile, Paths } from 'expo-file-system';
import { ImageManipulator, SaveFormat } from 'expo-image-manipulator';
import type { ImagePickerAsset } from 'expo-image-picker';
import * as Sharing from 'expo-sharing';
import { useSyncExternalStore } from 'react';
import { Platform } from 'react-native';
import { showToast } from '@/lib/toast';
import { api, ApiError, apiUpload, mediaUrl } from '@/lib/api/client';
import { authError } from '@/lib/api/auth';
import { queryClient } from '@/lib/api/query-client';
import type { Message, MessageMedia } from '@/lib/api/chat';
import { chatKeys } from '@/lib/api/chat';

/** Longest side of a photo sent in the chat (the server resizes again, to the same size). */
const MAX_SIDE = 1600;
/** Decoded at most this big before the resize: a 48 MP photo never sits whole in memory. */
const DECODE_MAX = 3200;
/** The server's cap on one upload. */
export const MAX_BYTES = 8 * 1024 * 1024;

/** A photo or a GIF on its message (anything else, or a media-less one from a newer server: its `text`). */
export const mediaOf = (m: Message): MessageMedia | null =>
  (m.kind === 'image' || m.kind === 'gif') && m.media?.url ? m.media : null;

/** A photo ready to send: a file on the phone (a blob: or data: URL on the web), its size, its type. */
export type ImageDraft = { uri: string; width: number; height: number; mime: string };

/**
 * A picked photo as it leaves the phone: at most 1600 px on its longest side, re-encoded as JPEG 0.8, which leaves its
 * EXIF (GPS, camera) behind. A GIF goes as it is (animated; GIF has no EXIF), within the server's 8 MB.
 */
export async function prepareImage(asset: ImagePickerAsset): Promise<ImageDraft | 'too_large'> {
  if (asset.mimeType === 'image/gif') {
    if (asset.fileSize && asset.fileSize > MAX_BYTES) return 'too_large';
    return { uri: asset.uri, width: asset.width || 1, height: asset.height || 1, mime: 'image/gif' };
  }
  let ref = await ImageManipulator.manipulate(asset.uri, { maxWidth: DECODE_MAX, maxHeight: DECODE_MAX }).renderAsync();
  if (Math.max(ref.width, ref.height) > MAX_SIDE) {
    const size = ref.width >= ref.height ? { width: MAX_SIDE } : { height: MAX_SIDE };
    ref = await ImageManipulator.manipulate(ref).resize(size).renderAsync();
  }
  const out = await ref.saveAsync({ compress: 0.8, format: SaveFormat.JPEG });
  return { uri: out.uri, width: out.width, height: out.height, mime: 'image/jpeg' };
}

type Ping = { mentions: number[]; everyone: boolean };

/** `POST /rounds/{id}/messages/image`, multipart: `image` (the file), `caption`, `mentions` (a JSON array), `everyone` ("1"). */
async function uploadImage(roundId: number, img: ImageDraft, caption: string, ping: Ping, clientKey: string) {
  const form = new FormData();
  form.append('client_key', clientKey);
  if (caption) form.append('caption', caption);
  if (ping.mentions.length) form.append('mentions', JSON.stringify(ping.mentions));
  if (ping.everyone) form.append('everyone', '1');
  const name = img.mime === 'image/gif' ? 'image.gif' : 'photo.jpg';
  if (Platform.OS === 'web') form.append('image', await (await fetch(img.uri)).blob(), name);
  // expo/fetch (the global fetch since SDK 58) can't send React Native's { uri } parts; a file-system File it can.
  else form.append('image', new FsFile(img.uri), name);
  return apiUpload<Message>(`/rounds/${roundId}/messages/image`, form, 'POST');
}

/** One GIF of a search, its URLs already through our server (`/api/gifs/media/…`). */
export type Gif = { id: string | number; title?: string | null; width: number; height: number; preview_url: string; url: string };
type GifPage = { items: Gif[]; next: boolean };

/** KLIPY's trending GIFs (empty `q`) or a search, page after page (`GET /gifs?q=&page=`, pages from 1). */
export function useGifs(q: string, enabled: boolean) {
  return useInfiniteQuery({
    queryKey: ['gifs', q],
    queryFn: ({ pageParam }) => api<GifPage>(`/gifs?q=${encodeURIComponent(q)}&page=${pageParam}`),
    initialPageParam: 1,
    getNextPageParam: (last, all) => (last?.next ? all.length + 1 : undefined),
    enabled,
    staleTime: 5 * 60_000,
  });
}

/** `POST /rounds/{id}/messages/gif` with the GIF as /gifs gave it. */
const postGif = (roundId: number, g: Gif, ping: Ping, clientKey: string) =>
  api<Message>(`/rounds/${roundId}/messages/gif`, {
    method: 'POST',
    json: {
      id: g.id,
      url: g.url,
      preview_url: g.preview_url,
      width: g.width,
      height: g.height,
      ...(ping.mentions.length ? { mentions: ping.mentions } : {}),
      ...(ping.everyone ? { everyone: true } : {}),
      client_key: clientKey,
    },
  });

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

const EXT: Record<string, string> = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp', 'image/gif': 'gif' };

/** The web can hand a file to the system's share sheet (otherwise the viewer's button downloads it). */
export const canShareFiles = () => Platform.OS !== 'web' || (typeof navigator !== 'undefined' && typeof navigator.canShare === 'function');

/**
 * Share or keep a chat photo / GIF. Phones: downloaded to the cache, then the share sheet (which offers to save it).
 * Web: the share sheet with the file when the browser has one, a download otherwise.
 */
export async function shareMedia(m: Message): Promise<void> {
  const media = mediaOf(m);
  if (!media) return;
  const url = mediaUrl(media.url);
  if (Platform.OS === 'web') {
    const blob = await (await fetch(url)).blob();
    const mime = blob.type || media.mime || (m.kind === 'gif' ? 'image/gif' : 'image/jpeg');
    const name = `qui-de-nous-${m.id}.${EXT[mime] ?? 'jpg'}`;
    if (typeof File !== 'undefined' && canShareFiles()) {
      const file = new File([blob], name, { type: mime });
      if (navigator.canShare({ files: [file] })) {
        try {
          await navigator.share({ files: [file] });
          return;
        } catch (e) {
          // Closing the share sheet is an answer, not a failure.
          if (e instanceof Error && e.name === 'AbortError') return;
        }
      }
    }
    const href = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = href;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(href), 30_000);
    return;
  }
  const mime = media.mime || (m.kind === 'gif' ? 'image/gif' : 'image/jpeg');
  const dir = new Directory(Paths.cache, 'chat');
  if (!dir.exists) dir.create({ intermediates: true });
  const file = new FsFile(dir, `qui-de-nous-${m.id}.${EXT[mime] ?? 'jpg'}`);
  if (!file.exists) await FsFile.downloadFileAsync(url, file, { idempotent: true });
  await Sharing.shareAsync(file.uri, { mimeType: mime });
}
