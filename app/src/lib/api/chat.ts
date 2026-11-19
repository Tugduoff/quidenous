import { useMutation, useQueryClient, useQuery, useInfiniteQuery } from '@tanstack/react-query';
import { api, apiUpload, readUriBlob } from '@/lib/api/client';
import { useSession } from '@/lib/session';
import { Platform } from 'react-native';
import { File as FsFile } from 'expo-file-system';
import type { ImageDraft } from '@/lib/device/media';
import { toggled } from '@/lib/utils/chat';

export const chatKeys = {
  all: ['messages'] as const,
  messages: (roundId: number) => ['messages', roundId] as const,
  gifs: (q: string) => ['gifs', q] as const,
};

const inFlight = new Map<number, number>();

type Vars = { id: number; emoji: string; on: boolean };

export function useReact(roundId: number) {
  const qc = useQueryClient();
  const { me } = useSession();
  const key = chatKeys.messages(roundId);
  const patch = (id: number, f: (m: Message) => Reaction[]) =>
    qc.setQueryData<Message[]>(key, (old) => old?.map((m) => (m.id === id ? { ...m, reactions: f(m) } : m)));
  const mutation = useMutation({
    mutationFn: ({ id, emoji, on }: Vars) =>
      api<{ message_id: number; reactions: Reaction[] }>(`/messages/${id}/reactions`, { method: on ? 'POST' : 'DELETE', json: { emoji } }),
    onMutate: ({ id, emoji, on }) => {
      inFlight.set(id, (inFlight.get(id) ?? 0) + 1);
      qc.cancelQueries({ queryKey: key });
      if (me) patch(id, (m) => toggled(m.reactions, emoji, me.id, on));
    },
    onSuccess: (res, { id }) => {
      if (inFlight.get(id) === 1 && res?.reactions) patch(id, () => res.reactions);
    },
    onError: (_e, { id, emoji, on }) => {
      if (me) patch(id, (m) => toggled(m.reactions, emoji, me.id, !on));
      qc.invalidateQueries({ queryKey: key });
    },
    onSettled: (_d, _e, { id }) => {
      const n = (inFlight.get(id) ?? 1) - 1;
      if (n > 0) inFlight.set(id, n);
      else inFlight.delete(id);
    },
  });
  return (m: Message, emoji: string) => {
    const on = !m.reactions?.some((r) => r.emoji === emoji && r.mine);
    mutation.mutate({ id: m.id, emoji, on });
    return on;
  };
}

export type Reaction = { emoji: string; count: number; mine: boolean; members: number[] };

export type MessageMedia = { url: string; width: number; height: number; mime?: string; preview_url?: string | null; provider?: string };

export type Message = {
  id: number;
  member_id: number;
  user_id?: number | null;
  text: string;
  created_at: string;
  mentions?: number[];
  everyone?: boolean;
  reactions?: Reaction[];
  kind?: 'text' | 'image' | 'gif' | (string & {});
  caption?: string | null;
  media?: MessageMedia | null;
  hidden?: boolean;
};

export const useMessages = (roundId: number) =>
  useQuery({ queryKey: chatKeys.messages(roundId), queryFn: () => api<Message[]>(`/rounds/${roundId}/messages`) });

export type NewMessage = { text: string; mentions?: number[]; everyone?: boolean; client_key: string };

export const postMessage = (roundId: number, message: NewMessage) => api<Message>(`/rounds/${roundId}/messages`, { method: 'POST', json: message });

export const deleteMessage = (roundId: number, id: number) => api(`/rounds/${roundId}/messages/${id}`, { method: 'DELETE' });

export const restoreMessage = (roundId: number, id: number) => api(`/rounds/${roundId}/messages/${id}/restore`, { method: 'POST' });

export type Ping = { mentions: number[]; everyone: boolean };

export async function uploadImage(roundId: number, img: ImageDraft, caption: string, ping: Ping, clientKey: string) {
  const form = new FormData();
  form.append('client_key', clientKey);
  if (caption) form.append('caption', caption);
  if (ping.mentions.length) form.append('mentions', JSON.stringify(ping.mentions));
  if (ping.everyone) form.append('everyone', '1');
  const name = img.mime === 'image/gif' ? 'image.gif' : 'photo.jpg';
  if (Platform.OS === 'web') form.append('image', await readUriBlob(img.uri), name);
  else form.append('image', new FsFile(img.uri), name);
  return apiUpload<Message>(`/rounds/${roundId}/messages/image`, form, 'POST');
}

export type Gif = { id: string | number; title?: string | null; width: number; height: number; preview_url: string; url: string };

type GifPage = { items: Gif[]; next: boolean };

export function useGifs(q: string, enabled: boolean) {
  return useInfiniteQuery({
    queryKey: chatKeys.gifs(q),
    queryFn: ({ pageParam }) => api<GifPage>(`/gifs?q=${encodeURIComponent(q)}&page=${pageParam}`),
    initialPageParam: 1,
    getNextPageParam: (last, all) => (last?.next ? all.length + 1 : undefined),
    enabled,
    staleTime: 5 * 60_000,
  });
}

export const postGif = (roundId: number, g: Gif, ping: Ping, clientKey: string) =>
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
