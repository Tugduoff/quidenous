import {
  AlarmClock,
  Anchor,
  Award,
  BellRing,
  CalendarDays,
  CalendarHeart,
  Coins,
  Copy,
  Crosshair,
  Dices,
  Eye,
  Crown,
  Feather,
  Fingerprint,
  Flame,
  Flower2,
  Gavel,
  Ghost,
  Gift,
  Handshake,
  Heart,
  Hourglass,
  Landmark,
  type LucideIcon,
  Medal,
  MessageCircle,
  MessageCircleQuestion,
  Moon,
  PartyPopper,
  Radar,
  RotateCcw,
  ShieldCheck,
  Shuffle,
  Snail,
  Sparkles,
  Split,
  Star,
  Sunrise,
  Swords,
  Target,
  Trophy,
  Turtle,
  Users,
  VolumeX,
  Zap,
  type LucideProps,
} from 'lucide-react-native';
import { createElement, type ReactNode } from 'react';
import { Text, View } from 'react-native';

import { formatNumber, t, type TKey } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

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

export const dec = (n: number) => formatNumber(Math.round(n * 10) / 10);

export const int = (n: number) => formatNumber(Math.round(n));

export const pct = (value: number, total: number | undefined) => t('stats.percent', { n: total ? Math.round((value / total) * 100) : 0 });

export const place = (n: number) => t('stats.place', { n: dec(n) });

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
  landmark: Landmark,
  fingerprint: Fingerprint,
  target: Target,
  eye: Eye,
  radar: Radar,
  'shield-check': ShieldCheck,
  'party-popper': PartyPopper,
  award: Award,
  'calendar-days': CalendarDays,
  'calendar-heart': CalendarHeart,
  coins: Coins,
  crosshair: Crosshair,
  dices: Dices,
  gift: Gift,
  'message-circle-question': MessageCircleQuestion,
  swords: Swords,
};

export const iconFor = (name: string | undefined): LucideIcon => (name && ICONS[name]) || Sparkles;

export const NamedIcon = ({ name, ...props }: LucideProps & { name: string | undefined }) => createElement(iconFor(name), props);

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

export const awardTitle = (key: string) => t(`stats.award.${key}` as TKey);

export function Caps({ children, color, size = 12, lines = 1 }: { children: ReactNode; color?: string; size?: number; lines?: number }) {
  const t = useTheme();
  return (
    <Text
      numberOfLines={lines}
      style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: size, letterSpacing: 0.6, color: color ?? t.mut, textTransform: 'uppercase' }}>
      {children}
    </Text>
  );
}

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <View style={{ gap: 12 }}>
      <Caps size={13}>{title}</Caps>
      {children}
    </View>
  );
}
