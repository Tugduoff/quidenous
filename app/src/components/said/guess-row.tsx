import { Text } from 'react-native';
import Animated, { FadeIn, FadeInDown, ReduceMotion, useAnimatedStyle, useReducedMotion, useSharedValue, withDelay, withTiming } from 'react-native-reanimated';
import { Avatar, AvatarStack } from '@/components/ui/avatar';
import type { Member } from '@/lib/api/group';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';
import { useT } from '@/lib/i18n';
import { EASE, LAYOUT_EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import type { guessGroups } from '@/lib/utils/said';

const sys = ReduceMotion.System;

export function GuessRow({
  group: g,
  byId,
  delay,
  settle,
  k = 1,
}: {
  group: ReturnType<typeof guessGroups>[number];
  byId: Map<number, Member>;
  delay: number;
  settle: number;
  k?: number;
}) {
  const t = useTheme();
  const tr = useT();
  const m = byId.get(g.member_id);
  const reduced = useReducedMotion();
  const fade = useSharedValue(1);
  useAnimateOnChange(fade, [g.right, settle, reduced], () =>
    g.right ? undefined : reduced ? 0.45 : withDelay(settle, withTiming(0.45, { duration: 260, easing: EASE })),
  );
  const dim = useAnimatedStyle(() => ({ opacity: fade.value }));
  return (
    <Animated.View entering={FadeInDown.delay(delay).duration(240).easing(LAYOUT_EASE).reduceMotion(sys)}>
      <Animated.View
        style={[
          {
            flexDirection: 'row',
            alignItems: 'center',
            gap: 10 * k,
            minHeight: 48 * k,
            paddingHorizontal: 10 * k,
            borderRadius: 14 * k,
            backgroundColor: t.card2,
          },
          dim,
        ]}>
        <Avatar member={m} size={Math.round(30 * k)} />
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: Math.round(15 * k), color: t.txt }}>
          {m?.name ?? '?'}
        </Text>
        <AvatarStack members={g.voters.map((id) => byId.get(id))} size={Math.round(22 * k)} ring={t.card2} max={4} />
        <Text
          accessibilityLabel={tr('said.result.found', { count: g.voters.length })}
          style={{ fontFamily: t.font.display, fontSize: Math.round(15 * k), color: t.mut }}>
          ×{g.voters.length}
        </Text>
        {g.right && (
          <Animated.View
            entering={FadeIn.delay(settle).duration(200).reduceMotion(sys)}
            pointerEvents="none"
            style={{
              position: 'absolute',
              left: -2,
              top: -2,
              right: -2,
              bottom: -2,
              borderRadius: 14 * k + 2,
              borderWidth: Math.round(2 * k),
              borderColor: t.accInk,
            }}
          />
        )}
      </Animated.View>
    </Animated.View>
  );
}
