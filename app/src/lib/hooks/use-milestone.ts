import { Platform } from 'react-native';
import { useSyncExternalStore, useState, useEffect } from 'react';
import { useCurrentRound } from '@/lib/api/rounds';
import { useGroup } from '@/lib/api/group';
import { storage } from '@/lib/device/storage';
import { lastMilestone } from '@/lib/utils/milestones';

const KEY = 'qdn.milestone';

const param = (k: string) =>
  Platform.OS === 'web' && typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get(k)) || null : null;

export const PREVIEW_AT = param('at');

let preview: number | null = param('fete') ?? param('milestone');

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

export function useMilestone() {
  const { data: round } = useCurrentRound();
  const room = useGroup().data?.id ?? null;
  const [stored, setStored] = useState<{ room: number; n: number } | null>(null);
  const seen = stored && stored.room === room ? stored.n : null;
  const finished = round && seen != null ? round.number - 1 : null;

  useEffect(() => {
    if (room == null) return;
    let live = true;
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
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setStored({ room, n: start });
    }
  }, [room, seen, finished]);

  const due = finished != null && seen != null && seen >= 0 ? lastMilestone(finished) : 0;
  const reached = seen != null && seen >= 0 && due > seen ? due : null;
  const dismiss = () => {
    if (reached == null || room == null) return;
    storage.set(`${KEY}.${room}`, String(reached));
    setStored({ room, n: reached });
  };
  return { milestone: reached, dismiss };
}
