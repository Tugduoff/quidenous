import type { ChooseResult } from '@/lib/api/rounds';

/** A wire from a to b: a cubic that leaves and arrives flat, sagging a little like a real cable. */
export function wirePath(ax: number, ay: number, bx: number, by: number) {
  'worklet';
  const dx = bx - ax;
  const sag = Math.min(26, 6 + Math.abs(dx) * 0.08 + Math.abs(by - ay) * 0.04);
  return `M ${ax} ${ay} C ${ax + dx * 0.45} ${ay + sag} ${bx - dx * 0.45} ${by + sag} ${bx} ${by}`;
}

/**
 * About how wide the items column wants to be: its longest label on two lines at most (bold 15 px is ~9 px a letter,
 * plus the padding and border of a plug). A rough guess is enough: labels wrap, and the column is clamped.
 */
export function itemsWidth(items: string[]) {
  const letters = Math.max(0, ...items.map((l) => Math.max(...l.split(/\s+/).map((w) => w.length), Math.ceil(l.length / 2))));
  return letters * 9 + 28;
}

export function measured<T>(list: readonly (T | undefined)[], count: number): T[] | null {
  const out: T[] = [];
  for (let k = 0; k < count; k++) {
    const item = list[k];
    if (!item) return null;
    out.push(item);
  }
  return out;
}

export type Geometry = {
  /** Row height, gap between rows, avatar size: compact as the group grows. */
  row: number;
  gap: number;
  avatar: number;
};

export type Rect = { y: number; h: number };

export const PAD = 10;
/** Where a wire leaves a person's row: the socket at its right end (from the row's right edge). */
export const SOCKET = 12;

export function geometry(n: number): Geometry {
  if (n <= 6) return { row: 54, gap: 8, avatar: 34 };
  if (n <= 12) return { row: 46, gap: 6, avatar: 30 };
  return { row: 40, gap: 4, avatar: 26 };
}

const ITEMS_MIN_SHARE = 0.24;
const ITEMS_MAX_SHARE = 0.33;
const WIRES_SHARE = 0.2;
const MIN_BOARD_H = 260;

/**
 * The wire board's layout for a width: items as narrow as their labels allow (a third of the board at most, the rest to
 * the names), a fifth of the board kept for the wires; tall enough for the items, and past `room` the people scroll.
 */
export function boardLayout(width: number, rows: number, items: string[], stackH: number, room: number) {
  const g = geometry(rows);
  const itemsW = Math.round(Math.min(width * ITEMS_MAX_SHARE, Math.max(width * ITEMS_MIN_SHARE, itemsWidth(items))));
  const itemsX = width - PAD - itemsW;
  const left = Math.round(itemsX - width * WIRES_SHARE + PAD);
  const anchorX = PAD + left - SOCKET;
  const content = PAD * 2 + rows * g.row + Math.max(0, rows - 1) * g.gap;
  const height = Math.max(stackH + PAD * 2, Math.min(content, Math.max(MIN_BOARD_H, room)));
  const stackY = Math.max(0, (height - stackH) / 2);
  return { g, itemsW, itemsX, left, anchorX, content, height, stackY };
}

export const rowTop = (g: Geometry, i: number) => PAD + i * (g.row + g.gap);

/** The item a wire let go at (x, y) plugs into: once near the items' column, the closest one. -1: none. */
export function hit(x: number, y: number, b: { itemsX: number; rects: Rect[] }) {
  'worklet';
  if (x < b.itemsX - 48 || !b.rects.length) return -1;
  let best = -1;
  let dist = Infinity;
  for (let k = 0; k < b.rects.length; k++) {
    const d = Math.abs(y - (b.rects[k].y + b.rects[k].h / 2));
    if (d < dist) {
      dist = d;
      best = k;
    }
  }
  return best;
}

/** Where each plugged wire ends: several wires into one item spread their plugs along its edge (at `x`). */
export function plugEnds(rects: Rect[], people: { id: number }[], choices: Record<number, number>, x: number) {
  const ends: Record<number, { x: number; y: number }> = {};
  rects.forEach((r, k) => {
    const into = people.filter((p) => choices[p.id] === k);
    into.forEach((p, j) => {
      const y = into.length === 1 ? r.y + r.h / 2 : r.y + 10 + ((r.h - 20) * j) / (into.length - 1);
      ends[p.id] = { x, y };
    });
  });
  return ends;
}

/** After wiring `sel` (row `i`) by taps: the next person still to wire, in order, wrapping around. */
export function nextToWire<P extends { id: number }>(people: P[], i: number, sel: number, choices: Record<number, number>): P | undefined {
  return [...people.slice(i + 1), ...people.slice(0, i)].find((p) => p.id !== sel && choices[p.id] == null);
}

export type ChoosePerson = ChooseResult['people'][number];

/** How a person's real choice compares with the group's guess. */
export function verdict(p: ChoosePerson): 'match' | 'surprise' | 'noGuess' | 'noAnswer' {
  if (p.real == null) return 'noAnswer';
  if (!p.top.length) return 'noGuess';
  return p.top.includes(p.real) ? 'match' : 'surprise';
}
