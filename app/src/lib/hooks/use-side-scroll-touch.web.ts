import { useEffect } from 'react';
import { type SharedValue, useSharedValue } from 'react-native-reanimated';

function landsInSideScroller(target: EventTarget | null) {
  for (let el = target instanceof Element ? target : null; el && el !== document.body; el = el.parentElement) {
    const overflowX = getComputedStyle(el).overflowX;
    if ((overflowX === 'auto' || overflowX === 'scroll') && el.scrollWidth > el.clientWidth) return true;
  }
  return false;
}

export function useSideScrollTouch(): SharedValue<boolean> {
  const inSideScroller = useSharedValue(false);
  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      inSideScroller.value = landsInSideScroller(e.target);
    };
    document.addEventListener('pointerdown', onPointerDown, true);
    return () => document.removeEventListener('pointerdown', onPointerDown, true);
  }, [inSideScroller]);
  return inSideScroller;
}
