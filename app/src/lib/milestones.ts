import { useEffect, useState } from 'react';
import { Platform } from 'react-native';

import { useCurrentRound } from './queries';
import { storage } from './storage';

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
export type Milestone = { n: number; theme: Theme; line: string; emoji?: string[]; epic?: boolean };

/** Numbers that get their own party, on top of every 50. */
const SPECIAL: Record<number, Omit<Milestone, 'n'>> = {
  10: { theme: 'confetti', line: 'Dix. L’échauffement est fini.' },
  42: { theme: 'emoji', emoji: ['🌌', '🐬', '🚀', '🪐'], line: 'La réponse à la grande question sur la vie, l’univers et le reste.' },
  69: { theme: 'emoji', emoji: ['😏'], line: 'Nice.' },
  100: { theme: 'gold', line: 'Vous vous connaissez trop bien.' },
  101: { theme: 'emoji', emoji: ['🐶', '🐾', '🐕'], line: '101 dalmatiens. Qui fait Cruella ?' },
  111: { theme: 'confetti', line: 'Que des 1. Joli.' },
  112: { theme: 'siren', emoji: ['🚨'], line: 'On appelle les urgences ?' },
  123: { theme: 'confetti', line: '1, 2, 3… soleil !' },
  180: { theme: 'emoji', emoji: ['🎯'], line: 'One hundred and eightyyy !' },
  214: { theme: 'hearts', emoji: ['💖', '💗', '💘', '❤️'], line: '14/02, en chiffres. Qui a un crush ?' },
  222: { theme: 'confetti', line: 'Que des 2.' },
  256: { theme: 'pixel', line: 'Un octet plein. Les geeks apprécieront.' },
  300: { theme: 'emoji', emoji: ['🎳'], line: 'Partie parfaite au bowling.' },
  314: { theme: 'emoji', emoji: ['🥧'], line: '3,14. Comme π. Et comme la tarte.' },
  321: { theme: 'rocket', line: '3, 2, 1… décollage !' },
  333: { theme: 'confetti', line: 'Que des 3. Moitié du diable, attention.' },
  365: { theme: 'emoji', emoji: ['📅', '🎂'], line: 'Une année de questions, si vous étiez assidus.' },
  404: { theme: 'glitch', line: 'Question introuvable. Ah si, la voilà.' },
  444: { theme: 'confetti', line: 'Que des 4.' },
  500: { theme: 'gold', line: 'La moitié de mille. Respect.' },
  512: { theme: 'pixel', line: 'Encore une puissance de 2.' },
  555: { theme: 'confetti', line: 'Que des 5. Tapez-vous dans la main.' },
  666: { theme: 'devil', line: 'Le nombre de la bête. Qui a vendu son âme ?', epic: true },
  777: { theme: 'jackpot', emoji: ['🪙', '💰', '🍒', '7️⃣'], line: 'Jackpot ! Vous avez gagné… une autre question.', epic: true },
  888: { theme: 'confetti', line: 'Que des 8. Infini, version debout.' },
  999: { theme: 'confetti', line: 'Que des 9. Le prochain, c’est le grand.' },
  1000: { theme: 'gold', line: 'MILLE. Vous êtes officiellement inséparables.', epic: true },
  1001: { theme: 'night', line: 'Les mille et une nuits.', epic: true },
  1024: { theme: 'pixel', line: 'Un kilo de questions (pour les geeks).', epic: true },
  1234: { theme: 'confetti', line: '1, 2, 3, 4. On recommence ?' },
  1337: { theme: 'matrix', line: 'Vous êtes des l33t h4x0rs.', epic: true },
  1515: { theme: 'tricolore', line: 'Marignan. La seule date que tout le monde retient.', epic: true },
  1664: { theme: 'emoji', emoji: ['🍺', '🍻'], line: 'Santé !' },
  1789: { theme: 'tricolore', line: 'Prise de la Bastille. Vive la révolution !', epic: true },
  2000: { theme: 'gold', line: 'Deux mille. Qui tient encore le compte ?', epic: true },
  2048: { theme: 'pixel', line: 'Vous avez gagné la partie.', epic: true },
};

const FIFTY: Record<number, string> = {
  50: 'Déjà. Ça devient sérieux.',
  150: 'Et toujours pas fâchés.',
  200: 'Il reste encore des secrets ?',
  250: 'Quelqu’un devrait écrire un livre.',
  300: 'Plus aucun mystère entre vous.',
};

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

export function milestone(n: number): Milestone {
  const special = SPECIAL[n];
  if (special) return { n, ...special };
  return { n, theme: 'confetti', line: FIFTY[n] ?? 'Toujours pas lassés ?' };
}

/** Web preview: `/?fete=666` plays a party without touching what this device has seen. */
export const PREVIEW =
  Platform.OS === 'web' && typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get('fete')) || null : null;

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

  const [preview, setPreview] = useState(PREVIEW);
  const due = finished != null && seen != null && seen >= 0 ? lastMilestone(finished) : 0;
  const reached = due > (seen ?? 0) ? due : null;
  const dismiss = () => {
    if (preview != null) return setPreview(null);
    if (reached == null) return;
    storage.set(KEY, String(reached));
    setSeen(reached);
  };
  return { milestone: preview ?? reached, dismiss };
}
