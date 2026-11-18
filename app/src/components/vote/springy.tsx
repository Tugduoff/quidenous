import type { ReactNode } from 'react';
import Animated from 'react-native-reanimated';
import { usePop } from '@/lib/motion';

/** The vote button bumps when it becomes usable (and each time the pick changes), so the next step is obvious. */
export function Springy({ trigger, when, children }: { trigger: unknown; when: boolean; children: ReactNode }) {
  return <Animated.View style={usePop(trigger, 1.04, when)}>{children}</Animated.View>;
}
