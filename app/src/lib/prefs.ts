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

/** First-run hints already closed (one per screen), so each shows until its X is tapped. Null until read. */
const TIPS_KEY = 'qdn.tips';
let tips: Set<string> | null = null;
const tipListeners = new Set<() => void>();
const tipsChanged = () => tipListeners.forEach((l) => l());

storage
  .get(TIPS_KEY)
  .then((saved) => {
    tips = new Set(saved ? (JSON.parse(saved) as string[]) : []);
  })
  .catch(() => {
    tips = new Set();
  })
  .finally(tipsChanged);

export function dismissTip(id: string) {
  tips = new Set([...(tips ?? []), id]);
  storage.set(TIPS_KEY, JSON.stringify([...tips])).catch(() => {});
  tipsChanged();
}

/** Whether the hint should show: false while the saved list is still loading, so a closed one never flashes. */
export function useTipVisible(id: string): boolean {
  return useSyncExternalStore(
    (l) => {
      tipListeners.add(l);
      return () => tipListeners.delete(l);
    },
    () => !!tips && !tips.has(id),
  );
}
