import Animated, { useReducedMotion, useSharedValue, useAnimatedStyle, withTiming, withSpring } from 'react-native-reanimated';
import { Pressable, type PressableProps, type StyleProp, type ViewStyle } from 'react-native';
import { SPRING } from '@/lib/motion';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

export function PressableScale({
  scale = 0.96,
  style,
  onPressIn,
  onPressOut,
  ...props
}: Omit<PressableProps, 'style'> & { style?: StyleProp<ViewStyle>; scale?: number }) {
  const reduced = useReducedMotion();
  const s = useSharedValue(1);
  const pressed = useAnimatedStyle(() => ({ transform: [{ scale: s.value }] }));
  return (
    <AnimatedPressable
      {...props}
      onPressIn={(e) => {
        if (!reduced) s.set(withTiming(scale, { duration: 90 }));
        onPressIn?.(e);
      }}
      onPressOut={(e) => {
        s.set(withSpring(1, SPRING));
        onPressOut?.(e);
      }}
      style={[style, pressed]}
    />
  );
}
