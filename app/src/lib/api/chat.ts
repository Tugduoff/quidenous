import { useMutation, useQueryClient, useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { useSession } from '@/lib/session';

export const chatKeys = {
  all: ['messages'] as const,
  messages: (roundId: number) => ['messages', roundId] as const,
  gifs: (q: string) => ['gifs', q] as const,
};

/** The reactions a chat message can get, in the order the server lists them. */
export const CHAT_REACTIONS = ['❤️', '😂', '😮', '😢', '👍', '🔥'] as const;
export const HEART = CHAT_REACTIONS[0];

const rank = (emoji: string) => {
  const i = (CHAT_REACTIONS as readonly string[]).indexOf(emoji);
  return i < 0 ? CHAT_REACTIONS.length : i;
};

/** The reactions once I add (`on`) or take back `emoji`: what the server will answer, shown before it does. */
export function toggled(list: Reaction[] | undefined, emoji: string, me: number, on: boolean): Reaction[] {
  const rest = (list ?? []).filter((r) => r.emoji !== emoji);
  const r = list?.find((x) => x.emoji === emoji);
  const members = (r?.members ?? []).filter((id) => id !== me);
  if (on) members.push(me);
  const next = members.length ? [...rest, { emoji, count: members.length, mine: on, members }] : rest;
  return next.sort((a, b) => rank(a.emoji) - rank(b.emoji));
}

/** Requests in flight per message: only the last answer is taken as is (an earlier one may land after it). */
const inFlight = new Map<number, number>();

type Vars = { id: number; emoji: string; on: boolean };

/** Add or take back my reaction on a message of `roundId`: shown at once, put back if the server says no. */
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
    // Undo just this one (other reactions may have changed meanwhile), then the server's word.
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
  /** Toggles `emoji` on `m`: adds it unless it is already mine. */
  return (m: Message, emoji: string) => {
    const on = !m.reactions?.some((r) => r.emoji === emoji && r.mine);
    mutation.mutate({ id: m.id, emoji, on });
    return on;
  };
}

/**
 * `user_id`: the author's account (null once deleted; absent from an older server).
 * `mentions`: member ids pinged with `@Name`; `everyone`: the text pings the whole room with `@tous` (both absent from an older server).
 */
/** One emoji on a chat message: how many, whether I'm among them, and who (member ids, oldest first). */
export type Reaction = { emoji: string; count: number; mine: boolean; members: number[] };

/**
 * A chat photo or GIF. `url` / `preview_url` are server paths (`/api/media/…`, `/api/gifs/media/…`: signed, short-lived,
 * no header needed) or absolute URLs; `mediaUrl()` makes them loadable.
 */
export type MessageMedia = { url: string; width: number; height: number; mime?: string; preview_url?: string | null; provider?: string };

export type Message = {
  id: number;
  member_id: number;
  user_id?: number | null;
  /** For a photo or a GIF: what an older app shows ("📷 Photo", "GIF", plus the caption). */
  text: string;
  created_at: string;
  mentions?: number[];
  everyone?: boolean;
  /** Absent from an older server. */
  reactions?: Reaction[];
  /** `text` when absent (an older server); a kind this app doesn't know shows its `text`. */
  kind?: 'text' | 'image' | 'gif' | (string & {});
  /** Photo or GIF: the text under it (mentions apply to it). */
  caption?: string | null;
  media?: MessageMedia | null;
  /** Hidden by reports: only the room owner gets it, to restore it. */
  hidden?: boolean;
};

export const useMessages = (roundId: number) =>
  useQuery({ queryKey: chatKeys.messages(roundId), queryFn: () => api<Message[]>(`/rounds/${roundId}/messages`) });
