import { View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withRepeat, withTiming } from 'react-native-reanimated';
import { EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';

/** While the first answer loads: the shape of the screen, breathing (still under reduced motion). */
export function TodaySkeleton() {
  const t = useTheme();
  const reduced = useReducedMotion();
  const o = useSharedValue(1);
  useAnimateOnChange(o, reduced, () => (reduced ? undefined : withRepeat(withTiming(0.55, { duration: 700, easing: EASE }), -1, true)));
  const breathe = useAnimatedStyle(() => ({ opacity: o.value }));
  const bar = (w: number | `${number}%`, h: number, bg = t.card2) => <View style={{ width: w, height: h, borderRadius: h / 2, backgroundColor: bg }} />;
  return (
    <Animated.View style={[{ gap: 16 }, breathe]} accessibilityElementsHidden importantForAccessibility="no-hide-descendants">
      <View style={{ gap: 10, marginTop: 4 }}>
        {bar('70%', 30, t.card)}
        {bar('45%', 14, t.card)}
      </View>
      {[0, 1, 2].map((i) => (
        <Card key={i} style={{ padding: 16, gap: 12 }}>
          {bar('35%', 12)}
          {bar('90%', 16)}
          {bar('60%', 16)}
          {bar(120, 30)}
        </Card>
      ))}
    </Animated.View>
  );
}
