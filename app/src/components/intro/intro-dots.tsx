import { View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, withSpring } from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { BOUNCE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

export function IntroDots({ index, count }: { index: number; count: number }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View accessible accessibilityLabel={tr('intro.step', { n: index + 1, total: count })} style={{ flexDirection: 'row', gap: 6 }}>
      {Array.from({ length: count }, (_, k) => (
        <Dot key={k} on={k === index} done={k < index} color={t.accInk} off={t.line} />
      ))}
    </View>
  );
}

function Dot({ on, done, color, off }: { on: boolean; done: boolean; color: string; off: string }) {
  const reduced = useReducedMotion();
  const style = useAnimatedStyle(() => ({
    width: reduced ? (on ? 20 : 8) : withSpring(on ? 20 : 8, BOUNCE),
    backgroundColor: on || done ? color : off,
  }));
  return <Animated.View style={[{ height: 8, borderRadius: 4 }, style]} />;
}
