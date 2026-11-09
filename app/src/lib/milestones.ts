import { useEffect, useState } from 'react';

import { useCurrentRound } from './queries';
import { storage } from './storage';

const KEY = 'qdn.milestone';

/** 50, 100, 150, 200… questions finished together. */
export const isMilestone = (n: number) => n >= 50 && n % 50 === 0;

/** Highest milestone at or below `n` (0 when none yet). */
export const lastMilestone = (n: number) => (n >= 50 ? n - (n % 50) : 0);

/** A funny line per milestone, a generic one past the list. */
export function milestoneLine(n: number) {
  const lines: Record<number, string> = {
    50: 'Déjà. Ça devient sérieux.',
    100: 'Vous vous connaissez trop bien.',
    150: 'Et toujours pas fâchés.',
    200: 'Il reste encore des secrets ?',
    250: 'Quelqu’un devrait écrire un livre.',
    300: 'Plus aucun mystère entre vous.',
  };
  return lines[n] ?? 'Toujours pas lassés ?';
}

/**
 * The milestone to celebrate on this device, once: the latest one the group has passed and this phone has not shown yet.
 * A fresh install starts from where the group is, so nobody gets an old party.
 */
export function useMilestone() {
  const { data: round } = useCurrentRound();
  const [seen, setSeen] = useState<number | null>(null);
  const finished = round ? round.number - 1 : null;

  useEffect(() => {
    storage.get(KEY).then((v) => setSeen(v == null ? -1 : Number(v)));
  }, []);

  useEffect(() => {
    if (seen === -1 && finished != null) {
      const start = lastMilestone(finished);
      storage.set(KEY, String(start));
      setSeen(start);
    }
  }, [seen, finished]);

  const due = finished != null && seen != null && seen >= 0 ? lastMilestone(finished) : 0;
  const milestone = due > (seen ?? 0) ? due : null;
  const dismiss = () => {
    if (milestone == null) return;
    storage.set(KEY, String(milestone));
    setSeen(milestone);
  };
  return { milestone, dismiss };
}
