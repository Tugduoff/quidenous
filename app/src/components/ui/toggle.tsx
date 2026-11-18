import type { LucideIcon } from 'lucide-react-native';
import { useTheme } from '@/lib/theme';
import Animated, { useSharedValue, withSpring, useAnimatedStyle, interpolateColor } from 'react-native-reanimated';
import { useEffect } from 'react';
import { SPRING } from '@/lib/motion';
import { Pressable, Text } from 'react-native';

/** A small on/off switch with its label, drawn to match the app (lime when on). An empty label shows the switch alone (`a11y` names it). */
export function Toggle({
  label,
  value,
  onChange,
  icon: Icon,
  a11y,
}: {
  label: string;
  value: boolean;
  onChange: (v: boolean) => void;
  icon?: LucideIcon;
  a11y?: string;
}) {
  const t = useTheme();
  const on = useSharedValue(value ? 1 : 0);
  useEffect(() => {
    on.value = withSpring(value ? 1 : 0, SPRING);
  }, [value, on]);
  // Plain strings: the worklets below copy what they use, and the theme object carries more than colours.
  const { card2, acc, line, mut, accTxt } = t;
  const track = useAnimatedStyle(() => ({
    backgroundColor: interpolateColor(on.value, [0, 1], [card2, acc]),
    borderColor: interpolateColor(on.value, [0, 1], [line, acc]),
  }));
  // 40 wide, minus the 1 px border and 3 px padding on each side, minus the 16 px thumb.
  const thumb = useAnimatedStyle(() => ({
    transform: [{ translateX: on.value * 16 }],
    backgroundColor: interpolateColor(on.value, [0, 1], [mut, accTxt]),
  }));
  return (
    <Pressable
      accessibilityRole="switch"
      accessibilityLabel={a11y ?? label}
      accessibilityState={{ checked: value }}
      onPress={() => onChange(!value)}
      hitSlop={8}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
      {Icon && <Icon size={14} color={value ? t.accInk : t.mut} style={{ marginRight: -3 }} />}
      {!!label && <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: value ? t.accInk : t.mut }}>{label}</Text>}
      <Animated.View style={[{ width: 40, height: 24, borderRadius: 12, padding: 3, borderWidth: 1, justifyContent: 'center' }, track]}>
        <Animated.View style={[{ width: 16, height: 16, borderRadius: 8 }, thumb]} />
      </Animated.View>
    </Pressable>
  );
}
