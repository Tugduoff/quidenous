import { useSyncExternalStore } from 'react';

import { storage } from '@/lib/device/storage';
import { cleanTheme, type RoomThemeId } from '@/lib/utils/room-theme';

const KEY = 'qdn.roomThemes';
let remembered: Record<string, RoomThemeId> = {};
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((l) => l());

storage
  .get(KEY)
  .then((saved) => {
    if (!saved) return;
    const parsed = JSON.parse(saved) as Record<string, string>;
    const next: Record<string, RoomThemeId> = {};
    for (const [room, id] of Object.entries(parsed)) {
      const clean = cleanTheme(id);
      if (clean) next[room] = clean;
    }
    remembered = { ...next, ...remembered };
    changed();
  })
  .catch(() => {});

export function rememberRoomTheme(room: number, id: RoomThemeId | null) {
  if ((remembered[room] ?? null) === id) return;
  const next = { ...remembered };
  if (id) next[room] = id;
  else delete next[room];
  remembered = next;
  storage.set(KEY, JSON.stringify(next)).catch(() => {});
  changed();
}

export function useRememberedRoomTheme(room: number | null): RoomThemeId | null {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => (room == null ? null : (remembered[room] ?? null)),
  );
}
