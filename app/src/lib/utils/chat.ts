import type { Message, MessageMedia, Reaction } from '@/lib/api/chat';

export const mediaOf = (m: Message): MessageMedia | null =>
  (m.kind === 'image' || m.kind === 'gif') && m.media?.url ? m.media : null;

export const CHAT_REACTIONS = ['❤️', '😂', '😮', '😢', '👍', '🔥'] as const;

export const HEART = CHAT_REACTIONS[0];

const rank = (emoji: string) => {
  const i = (CHAT_REACTIONS as readonly string[]).indexOf(emoji);
  return i < 0 ? CHAT_REACTIONS.length : i;
};

export function toggled(list: Reaction[] | undefined, emoji: string, me: number, on: boolean): Reaction[] {
  const rest = (list ?? []).filter((r) => r.emoji !== emoji);
  const r = list?.find((x) => x.emoji === emoji);
  const members = (r?.members ?? []).filter((id) => id !== me);
  if (on) members.push(me);
  const next = members.length ? [...rest, { emoji, count: members.length, mine: on, members }] : rest;
  return next.sort((a, b) => rank(a.emoji) - rank(b.emoji));
}

export const ratioOf = (w: number, h: number) => (w > 0 && h > 0 ? w / h : 1);

const FRAME_MAX_H = 320;
const FRAME_MIN_W = 120;
const FRAME_MAX_W = 300;
const FRAME_SCREEN_SHARE = 0.7;

export function frameSize(width: number, height: number, screen: number) {
  const ratio = ratioOf(width, height);
  let w = Math.min(screen * FRAME_SCREEN_SHARE, FRAME_MAX_W);
  let h = w / ratio;
  if (h > FRAME_MAX_H) {
    h = FRAME_MAX_H;
    w = Math.max(FRAME_MIN_W, h * ratio);
  }
  return { width: Math.round(w), maxWidth: '100%' as const, aspectRatio: w / h };
}

export function twoColumns<T extends { width: number; height: number }>(items: T[], colW: number, gap: number): T[][] {
  const cols: T[][] = [[], []];
  const heights = [0, 0];
  for (const g of items) {
    const c = heights[0] <= heights[1] ? 0 : 1;
    cols[c].push(g);
    heights[c] += colW / ratioOf(g.width, g.height) + gap;
  }
  return cols;
}

export function insertMention(text: string, typing: { at: number; query: string }, insert: string, max: number) {
  const after = text.slice(typing.at + 1 + typing.query.length).replace(/^\S*/, '');
  const head = `${text.slice(0, typing.at)}${insert} `;
  const next = (head + after.replace(/^ /, '')).slice(0, max);
  return { next, pos: Math.min(head.length, next.length) };
}
