import { useMutation, useQueryClient } from '@tanstack/react-query';

import { api, type Message, type Reaction } from '@/lib/api/client';
import { keys } from '@/lib/queries';
import { useSession } from '@/lib/session';

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
  const key = keys.messages(roundId);
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
