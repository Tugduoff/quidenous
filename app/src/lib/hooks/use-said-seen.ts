import { useSyncExternalStore } from 'react';
import { storage } from '@/lib/device/storage';

/** Rounds whose reveal I've watched (on this device): coming back shows the whole result at once. */
const SEEN_KEY = 'qdn.saidSeen';
const SEEN_KEPT = 60;
let seenIds: number[] | null = null;
const seenListeners = new Set<() => void>();
storage
  .get(SEEN_KEY)
  .then((saved) => {
    seenIds = saved ? (JSON.parse(saved) as number[]) : [];
  })
  .catch(() => {
    seenIds = [];
  })
  .finally(() => seenListeners.forEach((l) => l()));

export function markSaidSeen(roundId: number) {
  if (seenIds?.includes(roundId)) return;
  seenIds = [...(seenIds ?? []), roundId].slice(-SEEN_KEPT);
  storage.set(SEEN_KEY, JSON.stringify(seenIds)).catch(() => {});
  seenListeners.forEach((l) => l());
}

/** Seen, not seen, or null while the list loads (nothing drawn yet: no reveal flashing before the full result). */
export function useSaidSeen(roundId: number): boolean | null {
  return useSyncExternalStore(
    (l) => {
      seenListeners.add(l);
      return () => seenListeners.delete(l);
    },
    () => (seenIds ? seenIds.includes(roundId) : null),
  );
}
