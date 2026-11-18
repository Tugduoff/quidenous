import { router } from 'expo-router';
import { ChevronRight, Lock, Medal } from 'lucide-react-native';
import { memo, useId } from 'react';
import { Pressable, Text, View } from 'react-native';
import Svg, { Circle, Defs, RadialGradient, Stop } from 'react-native-svg';
import { useFeatures } from '@/lib/auth';
import { type TitleChip as Chip, type Tier, useBadges, useMemberTitle, useMyTitle } from '@/lib/badges';
import { type t as translate, useT } from '@/lib/i18n';
import { useScheme, useTheme } from '@/lib/theme';
import { iconFor } from '@/components/stats/common';
import { Card } from '@/components/ui/card';

/**
 * The three metals: `fill` the disc, `light` its shine, `rim` its edge, `ink` the tier's name as text (darker on the light
 * background, for 4.5:1). The icon on a disc is always near-black: every fill is light enough for it.
 */
export const TIERS = {
  1: { key: 'bronze', fill: '#D9935A', light: '#F4C79F', rim: '#9A5A2A', inkLight: '#8A4B1C', inkDark: '#E8A673' },
  2: { key: 'silver', fill: '#C3CBD6', light: '#F2F5F9', rim: '#7D8794', inkLight: '#56606E', inkDark: '#D5DCE5' },
  3: { key: 'gold', fill: '#F2C230', light: '#FFE89A', rim: '#B07F00', inkLight: '#805C00', inkDark: '#F5C542' },
} as const;
const DISC_INK = '#121216';

export const tierName = (tier: 1 | 2 | 3, tr: typeof translate) => tr(`badges.tiers.${TIERS[tier].key}`);

/** A tier's colour as text, readable on this theme's background. */
export function useTierInk(tier: 1 | 2 | 3) {
  return useScheme() === 'dark' ? TIERS[tier].inkDark : TIERS[tier].inkLight;
}

/**
 * A medal: a metal disc with the badge's icon once earned, a muted silhouette (and a small lock) while locked.
 * Decorative: whoever shows it says what it is.
 */
export const MedalDisc = memo(function MedalDisc({ icon, tier, size = 56 }: { icon: string; tier: Tier; size?: number }) {
  const t = useTheme();
  const id = useId().replace(/:/g, '');
  const Icon = iconFor(icon);
  const glyph = Math.round(size * 0.44);
  if (!tier) {
    return (
      <View
        importantForAccessibility="no-hide-descendants"
        accessibilityElementsHidden
        style={{
          width: size,
          height: size,
          borderRadius: size / 2,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: t.card2,
          borderWidth: Math.max(1.5, size / 28),
          borderColor: t.line,
          borderStyle: 'dashed',
        }}>
        <Icon size={glyph} color={t.mut} strokeWidth={2} opacity={0.55} />
        {size >= 40 && (
          <View
            style={{
              position: 'absolute',
              right: -2,
              bottom: -2,
              width: size * 0.36,
              height: size * 0.36,
              borderRadius: size * 0.18,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.card,
              borderWidth: 1,
              borderColor: t.line,
            }}>
            <Lock size={size * 0.18} color={t.mut} strokeWidth={2.5} />
          </View>
        )}
      </View>
    );
  }
  const m = TIERS[tier];
  const r = size / 2;
  const rim = Math.max(1.5, size / 18);
  return (
    <View importantForAccessibility="no-hide-descendants" accessibilityElementsHidden style={{ width: size, height: size }}>
      <Svg width={size} height={size} style={{ position: 'absolute' }}>
        <Defs>
          <RadialGradient id={`m${id}`} cx="35%" cy="30%" r="75%">
            <Stop offset="0" stopColor={m.light} />
            <Stop offset="0.6" stopColor={m.fill} />
            <Stop offset="1" stopColor={m.fill} />
          </RadialGradient>
        </Defs>
        <Circle cx={r} cy={r} r={r - rim / 2} fill={`url(#m${id})`} stroke={m.rim} strokeWidth={rim} />
        {size >= 32 && <Circle cx={r} cy={r} r={r - rim * 2.6} fill="none" stroke="#FFFFFF" strokeOpacity={0.45} strokeWidth={Math.max(1, rim / 2)} />}
      </Svg>
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
        <Icon size={glyph} color={DISC_INK} strokeWidth={2.25} />
      </View>
    </View>
  );
});

/** Three pips, filled up to the tier, each in its own metal. */
export function TierPips({ tier, size = 10 }: { tier: Tier; size?: number }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View accessible accessibilityLabel={tr('badges.a11y.tiers', { tier })} style={{ flexDirection: 'row', gap: size * 0.5 }}>
      {([1, 2, 3] as const).map((n) => (
        <View
          key={n}
          style={{
            width: size,
            height: size,
            borderRadius: size / 2,
            backgroundColor: n <= tier ? TIERS[n].fill : t.card2,
            borderWidth: 1,
            borderColor: n <= tier ? TIERS[n].rim : t.line,
          }}
        />
      ))}
    </View>
  );
}

/** A title next to a name: a small medal and the title, on one line. */
export const TitleChip = memo(function TitleChip({ chip, size = 'sm' }: { chip: Chip; size?: 'sm' | 'md' }) {
  const t = useTheme();
  const tr = useT();
  const md = size === 'md';
  return (
    <View
      accessible
      accessibilityLabel={tr('badges.a11y.chip', { name: chip.name, tier: tierName(chip.tier, tr) })}
      style={{
        alignSelf: 'flex-start',
        flexDirection: 'row',
        alignItems: 'center',
        gap: md ? 6 : 4,
        height: md ? 28 : 20,
        paddingLeft: 2,
        paddingRight: md ? 10 : 7,
        maxWidth: '100%',
        borderRadius: 999,
        backgroundColor: t.card2,
      }}>
      <MedalDisc icon={chip.icon} tier={chip.tier} size={md ? 24 : 16} />
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: md ? 13 : 11.5, color: t.txt }}>
        {chip.name}
      </Text>
    </View>
  );
});

/** A room member's displayed title (nothing when none, or on a server without titles). */
export const MemberTitle = memo(function MemberTitle({ id, size }: { id: number; size?: 'sm' | 'md' }) {
  const chip = useMemberTitle(id);
  return chip ? <TitleChip chip={chip} size={size} /> : null;
});

/** Profile, under my name: the title I display (tap: my collection). */
export function MyTitle() {
  const tr = useT();
  const chip = useMyTitle();
  if (!chip) return null;
  return (
    <Pressable accessibilityRole="button" accessibilityLabel={tr('badges.a11y.chip', { name: chip.name, tier: tierName(chip.tier, tr) })} onPress={() => router.push('/badges')} hitSlop={6}>
      <TitleChip chip={chip} size="md" />
    </Pressable>
  );
}

/** Profile row: « Titres et badges », with how many I have. */
export function BadgesRow() {
  const t = useTheme();
  const tr = useT();
  const { badges } = useFeatures();
  const { data } = useBadges();
  if (!badges) return null;
  return (
    <Pressable accessibilityRole="button" accessibilityLabel={`${tr('badges.title')}${data ? `, ${tr('badges.count', { earned: data.earned, total: data.total })}` : ''}`} onPress={() => router.push('/badges')}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
        <Medal size={20} color={t.txt} />
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('badges.title')}</Text>
        {data && (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut, fontVariant: ['tabular-nums'] }}>
            {data.earned}/{data.total}
          </Text>
        )}
        <ChevronRight size={20} color={t.mut} />
      </Card>
    </Pressable>
  );
}
