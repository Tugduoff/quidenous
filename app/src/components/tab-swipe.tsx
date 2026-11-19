import * as Haptics from 'expo-haptics';
import type { ReactNode } from 'react';
import { useEffect, useLayoutEffect, useRef } from 'react';
import { Platform, View } from 'react-native';
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

// Sideways travel that makes it a swipe, and up/down travel that makes it a scroll instead (checked first).
const ACTIVE_X = 18;
const FAIL_Y = 14;
// A finger held still this long is a long press (chat reactions, picking a row up, selecting text): never a swipe.
const SLOW = 250;
// Let go past this far, or flung this fast: the next page.
const DISTANCE = 64;
const FLING = 600;

/**
 * Web: whether the touch landed in something that scrolls sideways (the stats story carousel, the vote matrix). The
 * browser scrolls those natively, so gesture handler never hears about it and the swipe would change tab as well.
 */
function inSideScroller(target: EventTarget | null) {
  for (let el = target instanceof Element ? target : null; el && el !== document.body; el = el.parentElement) {
    const x = getComputedStyle(el).overflowX;
    if ((x === 'auto' || x === 'scroll') && el.scrollWidth > el.clientWidth) return true;
  }
  return false;
}

/**
 * Swiping sideways anywhere on the page: the next tab (leftwards) or the previous one (rightwards).
 * Activated by hand, from the touches, so that whatever else is under the finger wins first: a vertical scroll, a
 * horizontal ScrollView (it starts scrolling sooner; on the web, any touch inside one), a drag of its own, a long press. One finger, touch only: a mouse
 * drag still selects text on the web.
 */
export function TabSwipe({ onSwipe, children }: { onSwipe: (dir: 1 | -1) => void; children: ReactNode }) {
  const latest = useRef(onSwipe);
  useLayoutEffect(() => {
    latest.current = onSwipe;
  });
  const go = (dir: 1 | -1) => {
    if (Platform.OS !== 'web') Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
    latest.current(dir);
  };
  // Where and when the finger came down; `on` once it's a swipe (then only letting go matters).
  const start = useSharedValue({ x: 0, y: 0, at: 0, on: false });
  // Web: set on every touch, before the gesture sees it (capture phase, pointerdown comes first).
  const blocked = useSharedValue(false);
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    const down = (e: PointerEvent) => {
      blocked.value = inSideScroller(e.target);
    };
    document.addEventListener('pointerdown', down, true);
    return () => document.removeEventListener('pointerdown', down, true);
  }, [blocked]);
  const pan = usePanGesture({
    // Web: the browser keeps vertical scrolling, text selection and the long-press menu. In the gesture's config, not
    // as the detector's props: gesture handler 3.3 resets those once the gesture is set up (its config type leaves
    // them out, hence the cast below, but they're what the detector hands over; ignored on native).
    touchAction: 'pan-y',
    userSelect: 'auto',
    enableContextMenu: true,
    manualActivation: true,
    onTouchesDown: (e) => {
      'worklet';
      const p = e.allTouches[0];
      if (e.numberOfTouches > 1 || e.pointerType === PointerType.MOUSE || !p || blocked.value) {
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
      if (dy > FAIL_Y || Date.now() - start.value.at > SLOW) GestureStateManager.fail(e.handlerTag);
      else if (dx > ACTIVE_X && dx > dy * 2) {
        start.value = { ...start.value, on: true };
        GestureStateManager.activate(e.handlerTag);
      }
    },
    onDeactivate: (e) => {
      'worklet';
      if (e.canceled) return;
      const x = e.translationX;
      const flung = Math.abs(x) > ACTIVE_X * 2 && Math.abs(e.velocityX) > FLING && Math.sign(x) === Math.sign(e.velocityX);
      if (Math.abs(x) > DISTANCE || flung) scheduleOnRN(go, x < 0 ? 1 : -1);
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
