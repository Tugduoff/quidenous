import { Check } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { MedalDisc, TierPips, tierName } from '@/components/badges/title-chip';
import { BadgeProgress } from '@/components/badges/badge-progress';
import type { Badge } from '@/lib/api/badges';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { nextGoal } from '@/lib/utils/badges';

export function BadgeTile({ badge: b, width, shown, index, onOpen }: { badge: Badge; width: number; shown: boolean; index: number; onOpen: (b: Badge) => void }) {
  const t = useTheme();
  const tr = useT();
  const earned = b.tier > 0;
  const goal = nextGoal(b);
  const label = earned
    ? `${tr('badges.a11y.medal', { name: b.name, tier: tierName(b.tier as 1 | 2 | 3, tr) })}${shown ? `, ${tr('badges.a11y.displayed')}` : ''}`
    : tr('badges.a11y.medalLocked', { name: b.name, hint: b.hint });
  return (
    <Animated.View entering={motion.stagger(index)} style={{ width }}>
      <PressableScale
        scale={0.97}
        accessibilityRole="button"
        accessibilityLabel={label}
        onPress={() => onOpen(b)}
        style={[
          {
            alignItems: 'center',
            gap: 8,
            paddingVertical: 16,
            paddingHorizontal: 12,
            minHeight: 172,
            borderRadius: t.radius.card,
            backgroundColor: t.card,
            borderWidth: 2,
            borderColor: shown ? t.accInk : 'transparent',
          },
          t.shadow,
        ]}>
        <MedalDisc icon={b.icon} tier={b.tier} size={64} />
        <Text numberOfLines={2} style={{ textAlign: 'center', fontFamily: t.font.bold, fontSize: 14, lineHeight: 18, color: earned ? t.txt : t.mut }}>
          {b.name}
        </Text>
        {earned ? (
          <TierPips tier={b.tier} size={8} />
        ) : (
          <>
            <Text numberOfLines={2} style={{ textAlign: 'center', fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>
              {b.hint}
            </Text>
            <View style={{ flex: 1 }} />
            {goal != null && <BadgeProgress value={b.progress} goal={goal} />}
          </>
        )}
        {shown && (
          <View
            style={{
              position: 'absolute',
              top: 8,
              right: 8,
              width: 22,
              height: 22,
              borderRadius: 11,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.acc,
            }}>
            <Check size={13} color={t.accTxt} strokeWidth={3} />
          </View>
        )}
      </PressableScale>
    </Animated.View>
  );
}
