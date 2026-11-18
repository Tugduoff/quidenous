import { Platform } from 'react-native';
import { useSyncExternalStore, useState, useEffect } from 'react';
import { useCurrentRound } from '@/lib/api/rounds';
import { useGroup } from '@/lib/api/group';
import { storage } from '@/lib/device/storage';
import { lastMilestone } from '@/lib/utils/milestones';

/** One per room: `qdn.milestone.<roomId>`. The bare key is what older versions kept for every room at once. */
const KEY = 'qdn.milestone';

const param = (k: string) =>
  Platform.OS === 'web' && typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get(k)) || null : null;

/**
 * Hidden preview: `/?fete=666` (or `?milestone=666`) on the web, or the room name long-pressed five times, plays a party
 * without touching what this device has seen. `&at=1500` freezes it at that instant (captures).
 */
export const PREVIEW_AT = param('at');

let preview: number | null = param('fete') ?? param('milestone');

/** `/?serie=30`: the streak party instead. */
export const STREAK_PREVIEW = param('serie');

const previewListeners = new Set<() => void>();

export function playPreview(n: number | null) {
  preview = n;
  previewListeners.forEach((l) => l());
}

export function usePreview() {
  return useSyncExternalStore(
    (l) => {
      previewListeners.add(l);
      return () => previewListeners.delete(l);
    },
    () => preview,
  );
}

/**
 * The milestone to celebrate on this device, once per room: the latest one the room has passed and this phone has not shown yet.
 * A fresh install (or a room never opened here) starts from where the room is, so nobody gets an old party.
 */
export function useMilestone() {
  const { data: round } = useCurrentRound();
  const room = useGroup().data?.id ?? null;
  // What this device has shown, tagged with the room it was read for: after a switch it counts as unknown until read again.
  const [stored, setStored] = useState<{ room: number; n: number } | null>(null);
  const seen = stored && stored.room === room ? stored.n : null;
  const finished = round && seen != null ? round.number - 1 : null;

  useEffect(() => {
    if (room == null) return;
    let live = true;
    // No key for this room (the old shared one included) = fresh start for it.
    storage.get(`${KEY}.${room}`).then((v) => live && setStored({ room, n: v == null ? -1 : Number(v) }));
    storage.remove(KEY).catch(() => {});
    return () => {
      live = false;
    };
  }, [room]);

  useEffect(() => {
    if (room != null && seen === -1 && finished != null) {
      const start = lastMilestone(finished);
      storage.set(`${KEY}.${room}`, String(start));
      setStored({ room, n: start });
    }
  }, [room, seen, finished]);

  const due = finished != null && seen != null && seen >= 0 ? lastMilestone(finished) : 0;
  // Not read yet (or never set, as in a room with no question open): nothing to celebrate.
  const reached = seen != null && seen >= 0 && due > seen ? due : null;
  const dismiss = () => {
    if (reached == null || room == null) return;
    storage.set(`${KEY}.${room}`, String(reached));
    setStored({ room, n: reached });
  };
  return { milestone: reached, dismiss };
}
