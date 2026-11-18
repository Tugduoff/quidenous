import type { Message, MessageMedia, Reaction } from '@/lib/api/chat';

/** A photo or a GIF on its message (anything else, or a media-less one from a newer server: its `text`). */
export const mediaOf = (m: Message): MessageMedia | null =>
  (m.kind === 'image' || m.kind === 'gif') && m.media?.url ? m.media : null;

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
