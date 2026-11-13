import {
  AlarmClock,
  Anchor,
  BellRing,
  Copy,
  Crown,
  Feather,
  Flame,
  Flower2,
  Gavel,
  Ghost,
  Handshake,
  Heart,
  Hourglass,
  type LucideIcon,
  Medal,
  MessageCircle,
  Moon,
  RotateCcw,
  Shuffle,
  Snail,
  Sparkles,
  Split,
  Star,
  Sunrise,
  Trophy,
  Turtle,
  Users,
  VolumeX,
  Zap,
} from 'lucide-react-native';
import type { ReactNode } from 'react';
import { Text, View } from 'react-native';

import { formatNumber, t, type TKey } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** "45 s", "12 min", "3 h 20", "2 j 4 h" (in the current language). */
export function duration(seconds: number) {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return t('stats.duration.seconds', { count: s });
  const min = Math.round(s / 60);
  if (min < 60) return t('common.time.minutes', { count: min });
  const h = Math.floor(s / 3600);
  if (h < 24) {
    const m = Math.round((s % 3600) / 60);
    return m ? t('common.time.hoursMinutes', { h, mm: String(m).padStart(2, '0') }) : t('stats.duration.hours', { h });
  }
  const d = Math.floor(h / 24);
  return h % 24 ? t('stats.duration.daysHours', { d, h: h % 24 }) : t('stats.duration.days', { d });
}

/** One decimal at most, in the current language: 2.3 → "2,3" / "2.3". */
export const dec = (n: number) => formatNumber(Math.round(n * 10) / 10);

/** Thousands separated the local way ("1 234" / "1,234"). */
export const int = (n: number) => formatNumber(Math.round(n));

export const pct = (value: number, total: number | undefined) => t('stats.percent', { n: total ? Math.round((value / total) * 100) : 0 });

/** An average place: "2,3e" / "#2.3". */
export const place = (n: number) => t('stats.place', { n: dec(n) });

/** The server names icons in lucide's kebab-case; one this version doesn't know gets sparkles. */
const ICONS: Record<string, LucideIcon> = {
  star: Star,
  crown: Crown,
  'flower-2': Flower2,
  zap: Zap,
  snail: Snail,
  hourglass: Hourglass,
  'bell-ring': BellRing,
  'alarm-clock': AlarmClock,
  feather: Feather,
  heart: Heart,
  ghost: Ghost,
  shuffle: Shuffle,
  users: Users,
  anchor: Anchor,
  copy: Copy,
  gavel: Gavel,
  flame: Flame,
  trophy: Trophy,
  'rotate-ccw': RotateCcw,
  moon: Moon,
  sunrise: Sunrise,
  'message-circle': MessageCircle,
  'volume-x': VolumeX,
  medal: Medal,
  turtle: Turtle,
  split: Split,
  handshake: Handshake,
};

export const iconFor = (name: string | undefined): LucideIcon => (name && ICONS[name]) || Sparkles;

/** Every award key with its icon (docs/STATS.md); titles are `stats.award.<key>`. */
export const AWARD_ICONS: Record<string, string> = {
  most_chosen: 'star',
  most_crowned: 'crown',
  narcissist: 'flower-2',
  fastest: 'zap',
  slowest: 'snail',
  last_voter: 'hourglass',
  nudger: 'bell-ring',
  most_nudged: 'alarm-clock',
  author: 'feather',
  duo: 'heart',
  never_chosen: 'ghost',
  contrarian: 'shuffle',
  sheep: 'users',
  loyal: 'anchor',
  twins: 'copy',
  kingmaker: 'gavel',
  streak: 'flame',
  longest_streak: 'trophy',
  comeback: 'rotate-ccw',
  night_owl: 'moon',
  early_bird: 'sunrise',
  chatterbox: 'message-circle',
  silent: 'volume-x',
  best_rank: 'medal',
  worst_rank: 'turtle',
  most_split: 'split',
  most_unanimous: 'handshake',
  hottest: 'flame',
};

/** An award's title in the current language. */
export const awardTitle = (key: string) => t(`stats.award.${key}` as TKey);

/** A small-caps label: section headers, card titles. */
export function Caps({ children, color, size = 12 }: { children: ReactNode; color?: string; size?: number }) {
  const t = useTheme();
  return (
    <Text
      numberOfLines={1}
      style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: size, letterSpacing: 0.6, color: color ?? t.mut, textTransform: 'uppercase' }}>
      {children}
    </Text>
  );
}

/** A titled block of the stats page. */
export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <View style={{ gap: 12 }}>
      <Caps size={13}>{title}</Caps>
      {children}
    </View>
  );
}
