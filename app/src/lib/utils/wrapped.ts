import type { WrappedCard } from '@/lib/api/wrapped';

/** Cards with nobody on them: bold pairs of the app's accents, in turn. */
const PAIRS: [string, string][] = [
  ['#C8FF3D', '#5EF2B0'],
  ['#FF6DB5', '#FFB547'],
  ['#5AC8FA', '#B48CFF'],
  ['#FFB547', '#C8FF3D'],
  ['#B48CFF', '#FF6DB5'],
  ['#5EF2B0', '#5AC8FA'],
];

export const colorsOf = (card: WrappedCard, i: number): [string, string] => {
  const pair = PAIRS[i % PAIRS.length];
  const [p, q] = card.people;
  return p ? [p.color, q?.color ?? pair[1]] : pair;
};

/**
 * The big figure's size: a step by length, capped so a short figure stays on one line ("9 messages")
 * and a long one never breaks inside a word (~0.6 em per glyph of the display font, 320 pt wide).
 */
export const bigSize = (s: string) => {
  const step = s.length <= 3 ? 116 : s.length <= 6 ? 92 : s.length <= 10 ? 64 : s.length <= 16 ? 48 : 38;
  const fit = s.length <= 12 ? s.length : Math.max(...s.split(' ').map((w) => w.length));
  return Math.min(step, Math.floor(320 / (fit * 0.6)));
};
