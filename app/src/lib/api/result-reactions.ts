// Live reactions on results (docs/REACTIONS.md): the counts (react-query), the taps sent in batches, and the live
// events handed to whoever shows that result. The pure part (merging, the cap, applying taps) is in result-taps.ts.
import { type QueryClient, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback } from 'react';
import { api } from '@/lib/api/client';
import { addMine, applyLive, emptyReactions, type LiveTap, type ReactedEvent, type ResultReactions, type Tap, TapBatcher, tapsLeft, visibleTaps } from '@/lib/utils/result-taps';
import { useFeatures } from '@/lib/api/features';

export {
  RESULT_CAP,
  RESULT_REACTIONS,
  emojiIndex,
  burstList,
  type LiveTap,
  type ReactedEvent,
  type ResultReaction,
  type ResultReactions,
} from '@/lib/utils/result-taps';

const reactionKeys = {
  all: ['reactions'] as const,
  round: (roundId: number) => ['reactions', roundId] as const,
};

/** Per result: the taps waiting to go, and who I am there (so my own taps get my name before the server answers). */
const batchers = new Map<number, TapBatcher>();
const seats = new Map<number, number | null>();

/** What the server said plus my taps not sent yet (a refetch must not take back what I just tapped). */
const withQueued = (data: ResultReactions, roundId: number) => {
  const queued = batchers.get(roundId)?.queued ?? [];
  return queued.length ? addMine(data, queued, seats.get(roundId) ?? null) : data;
};

export function setReactionSeat(roundId: number, me: number | null) {
  seats.set(roundId, me);
}

function batcherFor(qc: QueryClient, roundId: number) {
  let b = batchers.get(roundId);
  if (b) return b;
  const key = reactionKeys.round(roundId);
  const batcher = new TapBatcher(async (taps: Tap[]) => {
    try {
      const res = await api<ResultReactions>(`/rounds/${roundId}/reactions`, { method: 'POST', json: { taps } });
      // The last answer on its way is the server's word (an earlier one may land after a later request was sent).
      if (res && batcher.inFlight === 1) qc.setQueryData<ResultReactions>(key, withQueued(res, roundId));
    } catch {
      // Refused (rate limit, the round is gone…): what the server has.
      qc.invalidateQueries({ queryKey: key });
    }
  });
  batchers.set(roundId, batcher);
  return batcher;
}

/**
 * A result's reactions, and `tap(emoji)`: one more, shown at once and sent with the taps of the next 250 ms. Returns
 * false when my cap is reached (nothing happens). `me`: my seat in the result's room.
 */
export function useReactionCounts(roundId: number, me: number | null, enabled = true) {
  const qc = useQueryClient();
  const { resultReactions } = useFeatures();
  const query = useQuery({
    queryKey: reactionKeys.round(roundId),
    queryFn: async () => withQueued(await api<ResultReactions>(`/rounds/${roundId}/reactions`), roundId),
    enabled: enabled && resultReactions,
    // Opening the result always asks again (live events keep it fresh while it is open).
    refetchOnMount: 'always',
  });
  const tap = useCallback(
    (emoji: string) => {
      const key = reactionKeys.round(roundId);
      const data = qc.getQueryData<ResultReactions>(key) ?? emptyReactions(roundId);
      if (tapsLeft(data) <= 0) return false;
      // A refetch on its way would take the tap back when it lands.
      qc.cancelQueries({ queryKey: key });
      qc.setQueryData<ResultReactions>(key, addMine(data, [{ emoji, count: 1 }], me));
      batcherFor(qc, roundId).add(emoji);
      return true;
    },
    [qc, roundId, me],
  );
  return { ...query, tap };
}

// Live events, handed to the screens showing that result.

export type ReactedListener = {
  /** My seat (my own taps are skipped: I played them as I tapped). */
  me: number | null;
  /** User ids I blocked (their taps are skipped). */
  blocked: ReadonlySet<number>;
  onTaps: (taps: LiveTap[]) => void;
};
const listeners = new Map<number, Set<ReactedListener>>();

/**
 * A `result_reacted` event (lib/live.ts, the big screen): the counts in the cache, the bursts to the screens showing
 * that result. Shown nowhere: its counts are only marked old, the next opening refetches them. Never refetches now.
 */
export function receiveReacted(qc: QueryClient, event: ReactedEvent) {
  if (typeof event.round_id !== 'number' || !Array.isArray(event.taps)) return;
  const key = reactionKeys.round(event.round_id);
  const subs = listeners.get(event.round_id);
  const first = subs?.values().next().value;
  if (!first) {
    qc.invalidateQueries({ queryKey: key, refetchType: 'none' });
    return;
  }
  const taps = visibleTaps(event.taps, first.me, first.blocked);
  if (!taps.length) return;
  qc.setQueryData<ResultReactions>(key, (old) => old && applyLive(old, taps));
  for (const l of subs) l.onTaps(taps);
}

export function listenReacted(roundId: number, l: ReactedListener): () => void {
  let set = listeners.get(roundId);
  if (!set) listeners.set(roundId, (set = new Set()));
  set.add(l);
  return () => {
    set.delete(l);
    if (!set.size) listeners.delete(roundId);
  };
}
