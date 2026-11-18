/** Streak milestones (7, 30, 100… days in a row): the generic party, with Paillette hopping in over the number. */
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';

import { Mascot } from '@/components/mascot/mascot';

import type { Clock } from '@/components/celebration/engine';
import { numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';
import { MEDIUM_SLAM, medium } from '@/components/celebration/scenes/medium';
import { small, smallLanding } from '@/components/celebration/scenes/small';

const SIZE = 150;

/** Pops up from behind the number at `at` (scene clock, ms), squashes on landing, then keeps hopping on its own. */
function HopIn({ clock, at, top, cx }: { clock: Clock; at: number; top: number; cx: number }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const T = [at, at + 180, at + 300, at + 420, at + 560];
    return {
      opacity: interpolate(c, [at, at + 60], [0, 1], Extrapolation.CLAMP),
      transform: [
        { translateY: interpolate(c, T, [SIZE * 0.6, -SIZE * 0.18, 0, 0, 0], Extrapolation.CLAMP) },
        { scaleX: interpolate(c, T, [0.4, 0.92, 1.16, 0.96, 1], Extrapolation.CLAMP) },
        { scaleY: interpolate(c, T, [0.4, 1.12, 0.84, 1.04, 1], Extrapolation.CLAMP) },
      ],
    };
  });
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: cx - SIZE / 2, top, transformOrigin: '50% 93%' }, a]}>
      <Mascot who="paillette" mood="celebrate" size={SIZE} />
    </Animated.View>
  );
}

export const streak: SceneFactory = (m, t) => {
  const base = (m.n >= 30 ? medium : small)(m, t);
  // When the number lands (small: rolled up; medium: slammed down).
  const at = m.n >= 30 ? MEDIUM_SLAM : smallLanding(m.n);
  const { Render } = base;
  return {
    ...base,
    Render: function Streak(p: SceneProps) {
      const box = numberSize(m.n) * 1.1;
      return (
        <>
          <Render {...p} />
          <HopIn clock={p.clock} at={at + 80} cx={p.cx} top={p.cy - box / 2 - SIZE * 0.9} />
        </>
      );
    },
  };
};
