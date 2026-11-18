import { Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withDelay, withSequence, withSpring, withTiming } from 'react-native-reanimated';
import { Avatar } from '@/components/ui/avatar';
import type { Member } from '@/lib/api/group';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';
import { BOUNCE, EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { FLIP_MS } from '@/lib/utils/said';

export function Flip({ member, at, size: S = 64 }: { member: Member | undefined; at: number; size?: number }) {
  const t = useTheme();
  const reduced = useReducedMotion();
  const p = useSharedValue(reduced ? 1 : 0);
  useAnimateOnChange(p, [at, reduced], () => (reduced ? undefined : withDelay(at, withTiming(1, { duration: FLIP_MS, easing: EASE }))));
  const pop = useSharedValue(1);
  useAnimateOnChange(pop, [at, reduced], () =>
    reduced ? undefined : withDelay(at + FLIP_MS, withSequence(withTiming(1.12, { duration: 110, easing: EASE }), withSpring(1, BOUNCE))),
  );
  const front = useAnimatedStyle(() => ({
    opacity: p.value < 0.5 ? 1 : 0,
    transform: [{ perspective: 600 }, { rotateY: `${p.value * 180}deg` }],
  }));
  const back = useAnimatedStyle(() => ({
    opacity: p.value < 0.5 ? 0 : 1,
    transform: [{ perspective: 600 }, { rotateY: `${p.value * 180 - 180}deg` }, { scale: pop.value }],
  }));
  return (
    <View style={{ width: S, height: S }}>
      <Animated.View
        style={[
          { position: 'absolute', width: S, height: S, borderRadius: S / 2, alignItems: 'center', justifyContent: 'center', backgroundColor: t.acc },
          front,
        ]}>
        <Text style={{ fontFamily: t.font.display, fontSize: Math.round(S * 0.47), color: t.accTxt }}>?</Text>
      </Animated.View>
      <Animated.View style={[{ position: 'absolute' }, back]}>
        <Avatar member={member} size={S} />
      </Animated.View>
    </View>
  );
}
