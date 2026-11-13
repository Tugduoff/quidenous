import { useEffect, useState, useSyncExternalStore } from 'react';
import { Platform } from 'react-native';

import { t, type TKey } from './i18n';
import { useCurrentRound, useGroup } from './queries';
import { storage } from './storage';

/** One per room: `qdn.milestone.<roomId>`. The bare key is what older versions kept for every room at once. */
const KEY = 'qdn.milestone';

export type Theme =
  | 'confetti'
  | 'emoji'
  | 'gold'
  | 'devil'
  | 'jackpot'
  | 'glitch'
  | 'matrix'
  | 'rocket'
  | 'hearts'
  | 'pixel'
  | 'siren'
  | 'tricolore'
  | 'night';
export type Milestone = { n: number; theme: Theme; line: string; epic?: boolean };

/** Numbers that get their own party, on top of every 50. Their punchline: `celebration.lines.m<n>`. */
const SPECIAL: Record<number, Omit<Milestone, 'n' | 'line'>> = {
  10: { theme: 'confetti' },
  42: { theme: 'emoji', epic: true },
  69: { theme: 'emoji', epic: true },
  100: { theme: 'gold' },
  101: { theme: 'emoji' },
  111: { theme: 'confetti' },
  112: { theme: 'siren' },
  123: { theme: 'confetti' },
  180: { theme: 'emoji' },
  214: { theme: 'hearts' },
  222: { theme: 'confetti' },
  256: { theme: 'pixel' },
  300: { theme: 'emoji' },
  314: { theme: 'emoji', epic: true },
  321: { theme: 'rocket' },
  333: { theme: 'confetti' },
  365: { theme: 'emoji' },
  404: { theme: 'glitch' },
  444: { theme: 'confetti' },
  500: { theme: 'gold' },
  512: { theme: 'pixel' },
  555: { theme: 'confetti' },
  666: { theme: 'devil', epic: true },
  777: { theme: 'jackpot', epic: true },
  888: { theme: 'confetti' },
  999: { theme: 'confetti' },
  1000: { theme: 'gold', epic: true },
  1001: { theme: 'night', epic: true },
  1024: { theme: 'pixel', epic: true },
  1234: { theme: 'confetti' },
  1337: { theme: 'matrix', epic: true },
  1515: { theme: 'tricolore', epic: true },
  1664: { theme: 'emoji' },
  1789: { theme: 'tricolore', epic: true },
  2000: { theme: 'gold', epic: true },
  2048: { theme: 'pixel', epic: true },
};

/** Every 50 with a punchline of its own (`celebration.fifty.f<n>`); the others get `celebration.other`. */
const FIFTY = [50, 150, 200, 250, 300];

export const isMilestone = (n: number) => (n >= 50 && n % 50 === 0) || n in SPECIAL;

/** Highest milestone at or below `n` (0 when none yet). */
export function lastMilestone(n: number) {
  const fifty = n >= 50 ? n - (n % 50) : 0;
  const special = Math.max(
    0,
    ...Object.keys(SPECIAL)
      .map(Number)
      .filter((k) => k <= n),
  );
  return Math.max(fifty, special);
}

/** How big the party is: every 50 is small; round hundreds and fun numbers medium; the legendary ones get their own scene. */
export type Tier = 'small' | 'medium' | 'epic';
export function tier(n: number): Tier {
  const special = SPECIAL[n];
  if (special?.epic) return 'epic';
  return special || n % 100 === 0 ? 'medium' : 'small';
}

/** Every number that has a party of its own, for the hidden preview. */
export const SPECIAL_NUMBERS = Object.keys(SPECIAL).map(Number);

export function milestone(n: number): Milestone {
  const special = SPECIAL[n];
  // Read at call time: the punchline follows the app's language.
  if (special) return { n, ...special, line: t(`celebration.lines.m${n}` as TKey) };
  return { n, theme: 'confetti', line: FIFTY.includes(n) ? t(`celebration.fifty.f${n}` as TKey) : t('celebration.other') };
}

const param = (k: string) =>
  Platform.OS === 'web' && typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get(k)) || null : null;

/**
 * Hidden preview: `/?fete=666` (or `?milestone=666`) on the web, or the room name long-pressed five times, plays a party
 * without touching what this device has seen. `&at=1500` freezes it at that instant (captures).
 */
export const PREVIEW_AT = param('at');
let preview: number | null = param('fete') ?? param('milestone');
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
