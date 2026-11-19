import { Text } from 'react-native';
import Animated, { type SharedValue, useAnimatedStyle, withSpring } from 'react-native-reanimated';
import { PressableScale } from '@/components/ui/pressable-scale';
import { SPRING, usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import type { Rect } from '@/lib/utils/choose';

export function ItemPlug({
  index,
  label,
  hovered,
  bump,
  armed,
  a11y,
  onLayout,
  onPress,
}: {
  index: number;
  label: string;
  hovered: SharedValue<number>;
  bump: number;
  armed: boolean;
  a11y: string;
  onLayout: (r: Rect) => void;
  onPress: () => void;
}) {
  const t = useTheme();
  const over = useAnimatedStyle(() => ({ transform: [{ scale: withSpring(hovered.value === index ? 1.06 : 1, SPRING) }] }));
  const snap = usePop(bump, 1.06, bump > 0);
  return (
    <Animated.View onLayout={(e) => onLayout({ y: e.nativeEvent.layout.y, h: e.nativeEvent.layout.height })} style={over}>
      <Animated.View style={snap}>
        <PressableScale
          testID={`choose-item-${index}`}
          accessibilityRole="button"
          accessibilityLabel={a11y}
          onPress={onPress}
          style={{
            minHeight: 52,
            justifyContent: 'center',
            paddingVertical: 10,
            paddingHorizontal: 12,
            borderRadius: 16,
            borderWidth: 2,
            borderColor: armed ? t.mut : t.line,
            backgroundColor: t.card2,
          }}>
          <Text numberOfLines={3} style={{ fontFamily: t.font.bold, fontSize: 15, lineHeight: 19, color: t.txt }}>
            {label}
          </Text>
        </PressableScale>
      </Animated.View>
    </Animated.View>
  );
}
