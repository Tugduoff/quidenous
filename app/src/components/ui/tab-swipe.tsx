import type { ReactNode } from 'react';
import { View } from 'react-native';
import {
  GestureDetector,
  GestureHandlerRootView,
  GestureStateManager,
  type PanGestureConfig,
  PointerType,
  usePanGesture,
} from 'react-native-gesture-handler';
import { useSharedValue } from 'react-native-reanimated';
import { scheduleOnRN } from 'react-native-worklets';
import { impactHaptic } from '@/lib/device/haptics';
import { useLatest } from '@/lib/hooks/use-latest';

const SWIPE_STARTS_AFTER_X = 18;
const SCROLL_WINS_AFTER_Y = 14;
const LONG_PRESS_NOT_SWIPE_MS = 250;
const NEXT_PAGE_DISTANCE = 64;
const NEXT_PAGE_FLING_VELOCITY = 600;

export function TabSwipe({ onSwipe, children }: { onSwipe: (dir: 1 | -1) => void; children: ReactNode }) {
  const latest = useLatest(onSwipe);
  const go = (dir: 1 | -1) => {
    impactHaptic();
    latest.current(dir);
  };
  const start = useSharedValue({ x: 0, y: 0, at: 0, on: false });
  const pan = usePanGesture({
    touchAction: 'pan-y',
    userSelect: 'auto',
    enableContextMenu: true,
    manualActivation: true,
    onTouchesDown: (e) => {
      'worklet';
      const p = e.allTouches[0];
      if (e.numberOfTouches > 1 || e.pointerType === PointerType.MOUSE || !p) {
        GestureStateManager.fail(e.handlerTag);
        return;
      }
      start.value = { x: p.absoluteX, y: p.absoluteY, at: Date.now(), on: false };
    },
    onTouchesMove: (e) => {
      'worklet';
      if (start.value.on) return;
      const p = e.allTouches[0];
      if (e.numberOfTouches > 1 || !p) {
        GestureStateManager.fail(e.handlerTag);
        return;
      }
      const dx = Math.abs(p.absoluteX - start.value.x);
      const dy = Math.abs(p.absoluteY - start.value.y);
      if (dy > SCROLL_WINS_AFTER_Y || Date.now() - start.value.at > LONG_PRESS_NOT_SWIPE_MS) GestureStateManager.fail(e.handlerTag);
      else if (dx > SWIPE_STARTS_AFTER_X && dx > dy * 2) {
        start.value = { ...start.value, on: true };
        GestureStateManager.activate(e.handlerTag);
      }
    },
    onDeactivate: (e) => {
      'worklet';
      if (e.canceled) return;
      const x = e.translationX;
      const flung = Math.abs(x) > SWIPE_STARTS_AFTER_X * 2 && Math.abs(e.velocityX) > NEXT_PAGE_FLING_VELOCITY && Math.sign(x) === Math.sign(e.velocityX);
      if (Math.abs(x) > NEXT_PAGE_DISTANCE || flung) scheduleOnRN(go, x < 0 ? 1 : -1);
    },
  } as PanGestureConfig);
  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <GestureDetector gesture={pan}>
        <View style={{ flex: 1 }}>{children}</View>
      </GestureDetector>
    </GestureHandlerRootView>
  );
}
