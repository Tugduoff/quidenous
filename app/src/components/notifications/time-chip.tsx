import { Text } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { hhmm } from '@/lib/utils/time';

export function TimeChip({ label, value, onPress }: { label: string; value: number; onPress: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale
      scale={0.97}
      accessibilityRole="button"
      accessibilityLabel={tr('notifications.timeA11y', { label, time: hhmm(value) })}
      onPress={onPress}
      style={{ flex: 1, height: 44, borderRadius: 14, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{hhmm(value)}</Text>
    </PressableScale>
  );
}
