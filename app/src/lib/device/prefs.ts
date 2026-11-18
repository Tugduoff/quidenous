import { useSyncExternalStore } from 'react';

import { storage } from '@/lib/device/storage';

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

export function useTipVisible(id: string): boolean {
  return useSyncExternalStore(
    (l) => {
      tipListeners.add(l);
      return () => tipListeners.delete(l);
    },
    () => !!tips && !tips.has(id),
  );
}
