import { useStreaks } from '@/lib/api/streaks';
import { useSession } from '@/lib/session';
import { useGroup } from '@/lib/api/group';
import { useState, useEffect } from 'react';
import { storage } from '@/lib/device/storage';
import { lastStreakMilestone, STREAK_MILESTONES } from '@/lib/utils/streaks';

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
      // eslint-disable-next-line react-hooks/set-state-in-effect
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
