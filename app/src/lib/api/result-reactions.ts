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

const batchers = new Map<number, TapBatcher>();
const seats = new Map<number, number | null>();

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
      if (res && batcher.inFlight === 1) qc.setQueryData<ResultReactions>(key, withQueued(res, roundId));
    } catch {
      qc.invalidateQueries({ queryKey: key });
    }
  });
  batchers.set(roundId, batcher);
  return batcher;
}

export function useReactionCounts(roundId: number, me: number | null, enabled = true) {
  const qc = useQueryClient();
  const { resultReactions } = useFeatures();
  const query = useQuery({
    queryKey: reactionKeys.round(roundId),
    queryFn: async () => withQueued(await api<ResultReactions>(`/rounds/${roundId}/reactions`), roundId),
    enabled: enabled && resultReactions,
    refetchOnMount: 'always',
  });
  const tap = useCallback(
    (emoji: string) => {
      const key = reactionKeys.round(roundId);
      const data = qc.getQueryData<ResultReactions>(key) ?? emptyReactions(roundId);
      if (tapsLeft(data) <= 0) return false;
      qc.cancelQueries({ queryKey: key });
      qc.setQueryData<ResultReactions>(key, addMine(data, [{ emoji, count: 1 }], me));
      batcherFor(qc, roundId).add(emoji);
      return true;
    },
    [qc, roundId, me],
  );
  return { ...query, tap };
}

export type ReactedListener = {
  me: number | null;
  blocked: ReadonlySet<number>;
  onTaps: (taps: LiveTap[]) => void;
};
const listeners = new Map<number, Set<ReactedListener>>();

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
