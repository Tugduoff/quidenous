import type { ReactNode } from 'react';
import Animated from 'react-native-reanimated';
import { usePop } from '@/lib/motion';

/** Bumps its child when `on` turns true: the face just picked, the place number just given. */
export function Pop({ on, to = 1.15, children }: { on: boolean; to?: number; children: ReactNode }) {
  return <Animated.View style={usePop(on, to, on)}>{children}</Animated.View>;
}
