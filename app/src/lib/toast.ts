import { useSyncExternalStore } from 'react';

export type Toast = { id: number; text: string };

let current: Toast | null = null;
let seq = 0;
const listeners = new Set<() => void>();
const set = (next: Toast | null) => {
  current = next;
  listeners.forEach((l) => l());
};

export function showToast(text: string) {
  set({ id: ++seq, text });
}

export function dismissToast(id?: number) {
  if (id == null || current?.id === id) set(null);
}

export const useToast = () =>
  useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => current,
  );
