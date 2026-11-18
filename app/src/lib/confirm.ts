import { useSyncExternalStore } from 'react';

export type ConfirmTone = 'danger' | 'acc';
export type Ask = { title: string; message?: string; action: string; tone?: ConfirmTone; resolve: (ok: boolean) => void };

let asking: Ask | null = null;
const listeners = new Set<() => void>();
const setAsking = (next: Ask | null) => {
  asking = next;
  listeners.forEach((l) => l());
};

export function askConfirm(title: string, message: string | undefined, action: string, tone?: ConfirmTone): Promise<boolean> {
  asking?.resolve(false);
  return new Promise((resolve) => setAsking({ title, message, action, tone, resolve }));
}

export function answerConfirm(ok: boolean) {
  asking?.resolve(ok);
  setAsking(null);
}

export const useAsking = () =>
  useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => asking,
  );
