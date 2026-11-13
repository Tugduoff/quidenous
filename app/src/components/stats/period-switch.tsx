import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import type { StatsPeriod } from '@/lib/api';
import { type TKey, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

const PERIODS: { value: StatsPeriod; label: TKey }[] = [
  { value: '7d', label: 'stats.period.week' },
  { value: '30d', label: 'stats.period.month' },
  { value: 'all', label: 'stats.period.all' },
];

/** Compact segmented control: the chosen period sits on an accent pill. */
export function PeriodSwitch({ value, onChange }: { value: StatsPeriod; onChange: (p: StatsPeriod) => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View accessibilityRole="radiogroup" style={{ flexDirection: 'row', padding: 3, borderRadius: t.radius.pill, backgroundColor: t.card2 }}>
      {PERIODS.map((p) => {
        const on = p.value === value;
        return (
          <Pressable
            key={p.value}
            accessibilityRole="radio"
            accessibilityState={{ checked: on }}
            onPress={() => onChange(p.value)}
            hitSlop={4}
            style={{ height: 30, paddingHorizontal: 11, borderRadius: t.radius.pill, alignItems: 'center', justifyContent: 'center' }}>
            {on && (
              <Animated.View
                entering={motion.enter}
                style={{ position: 'absolute', top: 0, bottom: 0, left: 0, right: 0, borderRadius: t.radius.pill, backgroundColor: t.acc }}
              />
            )}
            <Text style={{ fontFamily: on ? t.font.bold : t.font.semibold, fontSize: 13, color: on ? t.accTxt : t.mut }}>{tr(p.label)}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}
