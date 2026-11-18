import { View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';
import { EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';

/** Share of the group that has voted (0 to 1): fills up from empty, then grows with each new vote. */
export function VoteProgress({ value }: { value: number }) {
  const t = useTheme();
  const fill = useSharedValue(0);
  useAnimateOnChange(fill, value, () => withTiming(value, { duration: 450, easing: EASE }));
  const width = useAnimatedStyle(() => ({ width: `${fill.value * 100}%` }));
  return (
    <View style={{ height: 6, borderRadius: 3, backgroundColor: t.card2, overflow: 'hidden' }}>
      <Animated.View style={[{ height: 6, borderRadius: 3, backgroundColor: t.acc }, width]} />
    </View>
  );
}
