import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { api } from '@/lib/api/client';
import { useFeatures } from '@/lib/api/auth';
import { useSession } from '@/lib/session';
import { storage } from '@/lib/device/storage';
import { useGroup } from '@/lib/api/group';
import { roundKeys } from '@/lib/api/rounds';

/** Days in a row with a vote (docs/STREAKS.md): `current` (today counts once voted) and the best ever. */
export type Streak = { current: number; best: number };
/** `GET /streaks`: per current member (by id) and for the whole room; a party has none. */
export type Streaks = { members: Record<string, Streak>; room: Streak | null };

/** Shown next to a name from this many days (1 is just "voted today"). */
export const SHOWN_FROM = 2;
/** From this many days the flame takes the accent colour. */
export const HOT_FROM = 7;
/** My streak gets a party at these, once per room. */
export const STREAK_MILESTONES = [7, 30, 100] as const;

/** Under the open question's key: every vote, new question or arrival (live events) refreshes it with it. */
export const streakKeys = {
  room: [...roundKeys.current, 'streaks'] as const,
};

/** The room's streaks, from a server that has them (`streaks` in /api/features); never in a party. */
export function useStreaks() {
  const { streaks } = useFeatures();
  const { data: group } = useGroup();
  const on = streaks && !!group && group.kind !== 'party';
  const query = useQuery({ queryKey: streakKeys.room, queryFn: () => api<Streaks>('/streaks'), enabled: on, staleTime: 60_000 });
  return on ? query.data : undefined;
}

/** Someone's current streak in this room (0 when unknown). */
export function useMemberStreak(id: number | null | undefined) {
  const data = useStreaks();
  return id == null ? 0 : (data?.members[String(id)]?.current ?? 0);
}

/** Highest streak milestone at or below `n` (0 when none). */
export const lastStreakMilestone = (n: number) => Math.max(0, ...STREAK_MILESTONES.filter((m) => m <= n));

/** One per room: the highest streak milestone this device has celebrated. */
const KEY = 'qdn.streakMilestone';

/**
 * My streak milestone to celebrate, once per room and milestone. A room never seen here starts from where my streak is,
 * unless it just reached a milestone today (so nobody gets an old party).
 */
export function useStreakMilestone() {
  const data = useStreaks();
  const { me } = useSession();
  const room = useGroup().data?.id ?? null;
  const mine = me && data ? data.members[String(me.id)] : undefined;
  const [stored, setStored] = useState<{ room: number; n: number | null } | null>(null);
  const seen = stored && stored.room === room ? stored : null;

  useEffect(() => {
    if (room == null) return;
    let live = true;
    storage.get(`${KEY}.${room}`).then((v) => live && setStored({ room, n: v == null ? null : Number(v) }));
    return () => {
      live = false;
    };
  }, [room]);

  const current = mine?.current ?? 0;
  const due = lastStreakMilestone(current);
  useEffect(() => {
    // First time here: anything already passed counts as seen, except a milestone reached exactly now.
    if (room != null && seen && seen.n == null && mine) {
      const start = STREAK_MILESTONES.includes(current as (typeof STREAK_MILESTONES)[number]) ? lastStreakMilestone(current - 1) : due;
      storage.set(`${KEY}.${room}`, String(start));
      setStored({ room, n: start });
    }
  }, [room, seen, mine, current, due]);

  const reached = seen?.n != null && due > seen.n ? due : null;
  const dismiss = () => {
    if (reached == null || room == null) return;
    storage.set(`${KEY}.${room}`, String(reached));
    setStored({ room, n: reached });
  };
  return { milestone: reached, dismiss };
}
