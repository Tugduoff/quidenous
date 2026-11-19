import { View } from 'react-native';
import Animated, { useAnimatedStyle, withTiming } from 'react-native-reanimated';
import { EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

export function IntroProgress({ value }: { value: number }) {
  const t = useTheme();
  const style = useAnimatedStyle(() => ({ transform: [{ scaleX: withTiming(value, { duration: 260, easing: EASE }) }] }));
  return (
    <View style={{ width: 200, height: 8, borderRadius: 4, backgroundColor: t.card2, overflow: 'hidden' }}>
      <Animated.View style={[{ width: '100%', height: 8, borderRadius: 4, backgroundColor: t.accInk, transformOrigin: 'left' }, style]} />
    </View>
  );
}
