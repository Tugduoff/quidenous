import { type Lang, tIn, type TKey } from '@/lib/i18n-core';

type Theme =
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
export type Milestone = { n: number; theme: Theme; line: string; epic?: boolean; streak?: boolean; unit?: string };

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

const FIFTY = [50, 150, 200, 250, 300];

export const isMilestone = (n: number) => (n >= 50 && n % 50 === 0) || n in SPECIAL;

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

export type Tier = 'small' | 'medium' | 'epic';
export function tier(n: number): Tier {
  const special = SPECIAL[n];
  if (special?.epic) return 'epic';
  return special || n % 100 === 0 ? 'medium' : 'small';
}

export const SPECIAL_NUMBERS = Object.keys(SPECIAL).map(Number);

export function milestone(n: number, lang: Lang): Milestone {
  const special = SPECIAL[n];
  if (special) return { n, ...special, line: tIn(lang, `celebration.lines.m${n}` as TKey) };
  return { n, theme: 'confetti', line: FIFTY.includes(n) ? tIn(lang, `celebration.fifty.f${n}` as TKey) : tIn(lang, 'celebration.other') };
}

export function streakMilestone(n: number, lang: Lang): Milestone {
  const line = n >= 100 ? 'streaks.party.s100' : n >= 30 ? 'streaks.party.s30' : 'streaks.party.s7';
  return { n, theme: n >= 100 ? 'gold' : 'confetti', line: tIn(lang, line), streak: true, unit: tIn(lang, 'streaks.party.unit') };
}
