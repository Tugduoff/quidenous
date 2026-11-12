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

import { useTheme } from '@/lib/theme';

/** "45 s", "12 min", "3 h 20", "2 j 4 h". */
export function duration(seconds: number) {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s} s`;
  const min = Math.round(s / 60);
  if (min < 60) return `${min} min`;
  const h = Math.floor(s / 3600);
  if (h < 24) {
    const m = Math.round((s % 3600) / 60);
    return m ? `${h} h ${String(m).padStart(2, '0')}` : `${h} h`;
  }
  const d = Math.floor(h / 24);
  return h % 24 ? `${d} j ${h % 24} h` : `${d} j`;
}

export const plural = (n: number, one: string, many: string) => (n > 1 ? many : one);

/** French decimals: 2.3 → "2,3". */
export const dec = (n: number) => (Math.round(n * 10) / 10).toLocaleString('fr-FR');

/** Thousands with a thin space ("1 234"). */
export const int = (n: number) => Math.round(n).toLocaleString('fr-FR');

export const pct = (value: number, total: number | undefined) => `${total ? Math.round((value / total) * 100) : 0} %`;

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

/** Every award key with its title and icon (docs/STATS.md). */
export const AWARD_META: Record<string, { title: string; icon: string }> = {
  most_chosen: { title: 'La star', icon: 'star' },
  most_crowned: { title: 'Collection de couronnes', icon: 'crown' },
  narcissist: { title: 'Narcisse', icon: 'flower-2' },
  fastest: { title: 'Gâchette facile', icon: 'zap' },
  slowest: { title: 'Prend son temps', icon: 'snail' },
  last_voter: { title: 'Tout le monde attend', icon: 'hourglass' },
  nudger: { title: 'Relou officiel', icon: 'bell-ring' },
  most_nudged: { title: 'Toujours relancé', icon: 'alarm-clock' },
  author: { title: 'La plume', icon: 'feather' },
  duo: { title: 'Le crush', icon: 'heart' },
  never_chosen: { title: 'Les invisibles', icon: 'ghost' },
  contrarian: { title: 'Le contrarian', icon: 'shuffle' },
  sheep: { title: 'Le mouton', icon: 'users' },
  loyal: { title: 'Le fidèle', icon: 'anchor' },
  twins: { title: 'Les jumeaux de vote', icon: 'copy' },
  kingmaker: { title: 'Le faiseur de rois', icon: 'gavel' },
  streak: { title: 'Série en cours', icon: 'flame' },
  longest_streak: { title: 'Record de série', icon: 'trophy' },
  comeback: { title: 'Le revenant', icon: 'rotate-ccw' },
  night_owl: { title: 'Couche-tard', icon: 'moon' },
  early_bird: { title: 'Lève-tôt', icon: 'sunrise' },
  chatterbox: { title: 'Le bavard', icon: 'message-circle' },
  silent: { title: 'Le roi du silence', icon: 'volume-x' },
  best_rank: { title: 'Toujours sur le podium', icon: 'medal' },
  worst_rank: { title: 'Lanterne rouge', icon: 'turtle' },
  most_split: { title: 'La plus clivante', icon: 'split' },
  most_unanimous: { title: 'La plus unanime', icon: 'handshake' },
  hottest: { title: 'Le moment le plus chaud', icon: 'flame' },
};

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
