import type { WrappedCard } from '@/lib/api/wrapped';

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

const CARD_TEXT_WIDTH_PT = 320;
const DISPLAY_FONT_EM_PER_GLYPH = 0.6;

export const bigSize = (s: string) => {
  const step = s.length <= 3 ? 116 : s.length <= 6 ? 92 : s.length <= 10 ? 64 : s.length <= 16 ? 48 : 38;
  const fit = s.length <= 12 ? s.length : Math.max(...s.split(' ').map((w) => w.length));
  return Math.min(step, Math.floor(CARD_TEXT_WIDTH_PT / (fit * DISPLAY_FONT_EM_PER_GLYPH)));
};
