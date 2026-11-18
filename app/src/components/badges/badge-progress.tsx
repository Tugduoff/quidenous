import { View } from 'react-native';
import { useTheme } from '@/lib/theme';
import { clamp01 } from '@/lib/utils/easing';

export function BadgeProgress({ value, goal, height = 4 }: { value: number; goal: number; height?: number }) {
  const t = useTheme();
  const share = goal > 0 ? clamp01(value / goal) : 0;
  return (
    <View style={{ alignSelf: 'stretch', height, borderRadius: height / 2, backgroundColor: t.card2, overflow: 'hidden' }}>
      <View style={{ width: `${share * 100}%`, height, borderRadius: height / 2, backgroundColor: t.accInk }} />
    </View>
  );
}
