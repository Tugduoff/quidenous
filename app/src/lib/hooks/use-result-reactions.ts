import { useEffect } from 'react';
import { type LiveTap, listenReacted, setReactionSeat, useReactionCounts } from '@/lib/api/result-reactions';

export function useResultReactions(roundId: number, me: number | null, enabled = true) {
  useEffect(() => {
    setReactionSeat(roundId, me);
  }, [roundId, me]);
  return useReactionCounts(roundId, me, enabled);
}

export function useReactedListener(roundId: number | null, me: number | null, blocked: ReadonlySet<number>, onTaps: (taps: LiveTap[]) => void) {
  useEffect(() => {
    if (roundId == null) return;
    return listenReacted(roundId, { me, blocked, onTaps });
  }, [roundId, me, blocked, onTaps]);
}
