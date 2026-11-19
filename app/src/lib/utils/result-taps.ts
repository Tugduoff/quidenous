export const RESULT_REACTIONS = ['😂', '🔥', '😱', '👀', '💀', '❤️', '👏', '🙈'] as const;
export const RESULT_CAP = 30;
const BATCH_MS = 250;

export type Tap = { emoji: string; count: number };
export type LiveTap = Tap & { member_id: number; user_id: number };
export type ResultReaction = { emoji: string; count: number; mine: number; members: { member_id: number; count: number }[] };
export type ResultReactions = { round_id: number; cap: number; mine: number; accepted?: number; reactions: ResultReaction[] };
export type ReactedEvent = { type: 'result_reacted'; round_id: number; taps: LiveTap[] };

export const emojiIndex = (emoji: string) => (RESULT_REACTIONS as readonly string[]).indexOf(emoji);
const known = (emoji: string) => emojiIndex(emoji) >= 0;
const byPalette = (a: { emoji: string }, b: { emoji: string }) => emojiIndex(a.emoji) - emojiIndex(b.emoji);

export const emptyReactions = (roundId: number): ResultReactions => ({ round_id: roundId, cap: RESULT_CAP, mine: 0, reactions: [] });

export const tapsLeft = (data: ResultReactions | undefined) => Math.max(0, (data?.cap ?? RESULT_CAP) - (data?.mine ?? 0));

export function mergeTaps(...lists: Tap[][]): Tap[] {
  const sum = new Map<string, number>();
  for (const list of lists) for (const { emoji, count } of list) if (known(emoji) && count > 0) sum.set(emoji, (sum.get(emoji) ?? 0) + count);
  return [...sum].map(([emoji, count]) => ({ emoji, count })).sort(byPalette);
}

function addTaps(data: ResultReactions, taps: Tap[], member: number | null, mine: boolean): ResultReactions {
  const rows = new Map(data.reactions.map((r) => [r.emoji, r]));
  let added = 0;
  for (const { emoji, count } of taps) {
    if (!known(emoji) || count <= 0) continue;
    const r = rows.get(emoji) ?? { emoji, count: 0, mine: 0, members: [] };
    let members = r.members;
    if (member != null) {
      const i = members.findIndex((m) => m.member_id === member);
      members = i < 0 ? [...members, { member_id: member, count }] : members.map((m, j) => (j === i ? { ...m, count: m.count + count } : m));
    }
    rows.set(emoji, { ...r, count: r.count + count, mine: r.mine + (mine ? count : 0), members });
    added += count;
  }
  return { ...data, mine: data.mine + (mine ? added : 0), reactions: [...rows.values()].sort(byPalette) };
}

export function addMine(data: ResultReactions, taps: Tap[], me: number | null): ResultReactions {
  let left = tapsLeft(data);
  const within: Tap[] = [];
  for (const tap of mergeTaps(taps)) {
    const count = Math.min(tap.count, left);
    if (count > 0) within.push({ emoji: tap.emoji, count });
    left -= count;
  }
  return within.length ? addTaps(data, within, me, true) : data;
}

export function applyLive(data: ResultReactions, taps: LiveTap[]): ResultReactions {
  const byMember = new Map<number, LiveTap[]>();
  for (const tap of taps) byMember.set(tap.member_id, [...(byMember.get(tap.member_id) ?? []), tap]);
  let next = data;
  for (const [member, list] of byMember) next = addTaps(next, list, member, false);
  return next;
}

export const visibleTaps = (taps: LiveTap[], me: number | null, blocked: ReadonlySet<number>) =>
  taps.filter((x) => x.member_id !== me && !blocked.has(x.user_id) && known(x.emoji) && x.count > 0);

export function burstList(taps: Tap[], max: number): string[] {
  const left = taps.filter((x) => known(x.emoji)).map((x) => ({ emoji: x.emoji, n: Math.max(0, x.count) }));
  const out: string[] = [];
  while (out.length < max && left.some((x) => x.n > 0))
    for (const x of left)
      if (x.n > 0 && out.length < max) {
        out.push(x.emoji);
        x.n--;
      }
  return out;
}

type Timers = { set: (fn: () => void, ms: number) => unknown; clear: (handle: unknown) => void };
const realTimers: Timers = { set: (fn, ms) => setTimeout(fn, ms), clear: (h) => clearTimeout(h as ReturnType<typeof setTimeout>) };

export class TapBatcher {
  private queue = new Map<string, number>();
  private timer: unknown = null;
  inFlight = 0;

  constructor(
    private send: (taps: Tap[]) => Promise<unknown>,
    private delay = BATCH_MS,
    private timers: Timers = realTimers,
  ) {}

  add(emoji: string, count = 1) {
    if (!known(emoji) || count <= 0) return;
    this.queue.set(emoji, (this.queue.get(emoji) ?? 0) + count);
    if (this.timer == null) this.timer = this.timers.set(() => void this.flush().catch(() => {}), this.delay);
  }

  get queued(): Tap[] {
    return mergeTaps([...this.queue].map(([emoji, count]) => ({ emoji, count })));
  }

  get idle() {
    return this.inFlight === 0 && this.queue.size === 0;
  }

  async flush() {
    if (this.timer != null) this.timers.clear(this.timer);
    this.timer = null;
    const taps = this.queued;
    this.queue.clear();
    if (!taps.length) return;
    this.inFlight++;
    try {
      await this.send(taps);
    } finally {
      this.inFlight--;
    }
  }
}
