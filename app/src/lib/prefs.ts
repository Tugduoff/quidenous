import { useSyncExternalStore } from 'react';

import { storage } from './storage';

/** Hide who I voted for on the waiting screen, for when friends can see my phone. */
const KEY = 'qdn.hideVote';
let hideVote = false;
const listeners = new Set<() => void>();

storage.get(KEY).then((saved) => {
  if (saved === '1') setHideVote(true);
});

export function setHideVote(next: boolean) {
  hideVote = next;
  storage.set(KEY, next ? '1' : '0').catch(() => {});
  listeners.forEach((l) => l());
}

export function useHideVote(): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => hideVote,
  );
}
