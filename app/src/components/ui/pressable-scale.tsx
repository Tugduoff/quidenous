import Animated, { useReducedMotion, useSharedValue, useAnimatedStyle, withTiming, withSpring } from 'react-native-reanimated';
import { Pressable, type PressableProps, type StyleProp, type ViewStyle } from 'react-native';
import { SPRING } from '@/lib/motion';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/** A Pressable that dips a little while held: the app's press feedback (none under reduced motion). */
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
        if (!reduced) s.value = withTiming(scale, { duration: 90 });
        onPressIn?.(e);
      }}
      onPressOut={(e) => {
        s.value = withSpring(1, SPRING);
        onPressOut?.(e);
      }}
      style={[style, pressed]}
    />
  );
}
