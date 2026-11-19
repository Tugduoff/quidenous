import { Text } from 'react-native';
import Animated, { useAnimatedStyle, withTiming } from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { type Who } from '@/lib/utils/mascot';
import { EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

export function IntroCandidate({ who, picked, dim, onPress }: { who: Who; picked: boolean; dim: boolean; onPress?: () => void }) {
  const t = useTheme();
  const tr = useT();
  const name = tr(`intro.names.${who}`);
  const style = useAnimatedStyle(() => ({ opacity: withTiming(dim ? 0.4 : 1, { duration: 200, easing: EASE }) }));
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('intro.vote.pickA11y', { name })}
      accessibilityState={{ selected: picked, disabled: !onPress }}
      disabled={!onPress}
      onPress={onPress}
      style={{ width: '30%', minWidth: 96, maxWidth: 140 }}>
      <Animated.View
        style={[
          { alignItems: 'center', gap: 4, paddingTop: 8, paddingBottom: 10, borderRadius: 18, backgroundColor: t.card, borderWidth: 2, borderColor: picked ? t.accInk : 'transparent' },
          t.shadow,
          style,
        ]}>
        <Mascot who={who} mood={picked ? 'celebrate' : 'happy'} size={72} animate={picked} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{name}</Text>
      </Animated.View>
    </PressableScale>
  );
}
