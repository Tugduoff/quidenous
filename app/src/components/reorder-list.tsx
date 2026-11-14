import * as Haptics from 'expo-haptics';
import { type ComponentRef, type ReactNode, type RefObject, useEffect, useRef, useState } from 'react';
import { Dimensions, type NativeScrollEvent, Platform, type ScrollView, View, type ViewStyle } from 'react-native';
import { GestureDetector, GestureHandlerRootView, usePanGesture } from 'react-native-gesture-handler';
import Animated, { ReduceMotion, type SharedValue, useAnimatedReaction, useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';
import { scheduleOnRN } from 'react-native-worklets';

import { EASE, motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

// A list whose rows can be put in another order: hold one (~300 ms), it lifts, drag it, the others make room.
// Rows are absolutely positioned from their measured heights, so a drop never waits on React to re-lay them out.

/** Same gap as the Screen's cards. */
const GAP = 16;
const HOLD = 300;
/** Auto-scroll: how close to the edge of the scroll view it starts, and its top speed (px per frame). */
const EDGE = 90;
const SPEED = 14;
const SLIDE = { duration: 200, easing: EASE, reduceMotion: ReduceMotion.System } as const;
const UNPLACED = -1e6;
/** Web: holding a row with the mouse doesn't select its text (nor open the iOS callout). */
const NO_SELECT = Platform.OS === 'web' ? ({ userSelect: 'none', WebkitTouchCallout: 'none' } as unknown as ViewStyle) : null;
/** Mobile web: a held row keeps the finger, the page doesn't scroll away under it (that would cancel the drag). */
const holdPage = (e: Event) => e.preventDefault();
function pageScroll(on: boolean) {
  if (Platform.OS !== 'web' || typeof document === 'undefined') return;
  if (on) document.removeEventListener('touchmove', holdPage);
  else document.addEventListener('touchmove', holdPage, { passive: false });
}

type Layout = { h: Record<number, number>; top: Record<number, number> };

type Shared = {
  order: SharedValue<number[]>;
  layout: SharedValue<Layout>;
  /** The row in hand (or settling after the drop), -1: none. */
  active: SharedValue<number>;
  lifted: SharedValue<boolean>;
  startTop: SharedValue<number>;
  /** The finger's travel since the lift, then the slide into its slot. */
  trans: SharedValue<number>;
  /** How far the page auto-scrolled since the lift. */
  scrolled: SharedValue<number>;
  /** The finger, in window coordinates (for auto-scroll). */
  pointer: SharedValue<number>;
};

/** Each row's top: one after the other; a row not measured yet takes no room. */
function tops(order: number[], h: Record<number, number>) {
  'worklet';
  const top: Record<number, number> = {};
  let y = 0;
  for (const id of order) {
    top[id] = y;
    if (h[id] !== undefined) y += h[id] + GAP;
  }
  return top;
}

function total(order: number[], h: Record<number, number>) {
  const hs = order.map((id) => h[id]).filter((x) => x !== undefined);
  return hs.reduce((a, b) => a + b, 0) + Math.max(0, hs.length - 1) * GAP;
}

/** The page's scroll position and content height, for auto-scroll: hand `onScroll` / `onContentSizeChange` to the Screen. */
export function useScrollTracker() {
  const state = useRef({ y: 0, content: 0, locked: false });
  return {
    state,
    onScroll: (e: NativeScrollEvent) => {
      if (!state.current.locked) state.current.y = e.contentOffset.y;
    },
    onContentSizeChange: (_w: number, h: number) => {
      state.current.content = h;
    },
  };
}

export type ScrollTracker = ReturnType<typeof useScrollTracker>;

export function ReorderList<T extends { id: number }>({
  items,
  render,
  onReorder,
  onStill,
  onDragChange,
  scroller,
  tracker,
  isNew,
}: {
  items: T[];
  render: (item: T) => ReactNode;
  /** Dropped somewhere else: every id, in the new order. */
  onReorder: (ids: number[]) => void;
  /** Held and let go without moving. */
  onStill?: (id: number) => void;
  onDragChange?: (dragging: boolean) => void;
  scroller: RefObject<ComponentRef<typeof ScrollView> | null>;
  tracker: ScrollTracker;
  /** Rows added after the list first showed fade in. */
  isNew?: (id: number) => boolean;
}) {
  const ids = items.map((i) => i.id);
  const key = ids.join(',');
  const latest = useRef(ids);
  latest.current = ids;
  const heights = useRef<Record<number, number>>({});
  const [height, setHeight] = useState(0);
  const dragging = useRef(false);
  const s: Shared = {
    order: useSharedValue(ids),
    layout: useSharedValue<Layout>({ h: {}, top: {} }),
    active: useSharedValue(-1),
    lifted: useSharedValue(false),
    startTop: useSharedValue(0),
    trans: useSharedValue(0),
    scrolled: useSharedValue(0),
    pointer: useSharedValue(0),
  };

  // React's order and the measured heights → where each row sits (left alone while a row is in hand).
  const sync = () => {
    if (dragging.current) return;
    const h = heights.current;
    s.order.value = latest.current;
    s.layout.value = { h, top: tops(latest.current, h) };
    setHeight(total(latest.current, h));
  };
  useEffect(() => {
    // The row in hand went away (deleted meanwhile): let go of it.
    if (dragging.current && !latest.current.includes(s.active.value)) {
      dragging.current = false;
      s.lifted.value = false;
      s.active.value = -1;
      tracker.state.current.locked = false;
      pageScroll(true);
      onDragChange?.(false);
    }
    sync();
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps
  // Heights arrive one row at a time: applied together on the next frame, so the first layout doesn't cascade.
  const frame = useRef<number | null>(null);
  const measured = (id: number, h: number) => {
    if (heights.current[id] === h) return;
    heights.current = { ...heights.current, [id]: h };
    if (frame.current == null)
      frame.current = requestAnimationFrame(() => {
        frame.current = null;
        sync();
      });
  };

  // While a row is in hand: find its slot from where it is, the others slide out of the way.
  useAnimatedReaction(
    () => (s.lifted.value ? s.startTop.value + s.trans.value + s.scrolled.value : null),
    (top) => {
      if (top == null) return;
      const id = s.active.value;
      const { h } = s.layout.value;
      const own = h[id] ?? 0;
      const center = top + own / 2;
      const others = s.order.value.filter((x) => x !== id);
      // Before `k` or after it: the switch is halfway between the two places it would take.
      let at = 0;
      let y = 0;
      for (const k of others) {
        const hk = h[k] ?? 0;
        if (center > y + (own + hk + GAP) / 2) at++;
        else break;
        y += hk + GAP;
      }
      const next = [...others.slice(0, at), id, ...others.slice(at)];
      if (next.join(',') === s.order.value.join(',')) return;
      s.order.value = next;
      s.layout.value = { h, top: tops(next, h) };
    },
  );

  // Auto-scroll on the JS side (the Screen's ScrollView is a plain one), the row in hand follows.
  const loop = useRef<number | null>(null);
  const startScroll = () => {
    const view = { top: 0, height: Dimensions.get('window').height };
    const sv = scroller.current as unknown as { measureInWindow?: (cb: (x: number, y: number, w: number, h: number) => void) => void } | null;
    sv?.measureInWindow?.((_x, y, _w, h) => {
      if (h > 0) Object.assign(view, { top: y, height: h });
    });
    const st = tracker.state.current;
    st.locked = true;
    const start = st.y;
    const tick = () => {
      if (!dragging.current) return;
      const p = s.pointer.value;
      let v = 0;
      if (p < view.top + EDGE) v = -SPEED * Math.min(1, (view.top + EDGE - p) / EDGE);
      else if (p > view.top + view.height - EDGE) v = SPEED * Math.min(1, (p - (view.top + view.height - EDGE)) / EDGE);
      if (v) {
        const max = Math.max(0, st.content - view.height);
        const next = Math.max(0, Math.min(max, st.y + v));
        if (next !== st.y) {
          st.y = next;
          scroller.current?.scrollTo({ y: next, animated: false });
          s.scrolled.value = next - start;
        }
      }
      loop.current = requestAnimationFrame(tick);
    };
    loop.current = requestAnimationFrame(tick);
  };
  useEffect(
    () => () => {
      if (loop.current != null) cancelAnimationFrame(loop.current);
      if (frame.current != null) cancelAnimationFrame(frame.current);
      pageScroll(true);
    },
    [],
  );

  const lift = () => {
    dragging.current = true;
    onDragChange?.(true);
    pageScroll(false);
    if (Platform.OS !== 'web') Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
    startScroll();
  };
  const drop = (id: number, order: number[], moved: boolean) => {
    dragging.current = false;
    pageScroll(true);
    if (loop.current != null) cancelAnimationFrame(loop.current);
    tracker.state.current.locked = false;
    onDragChange?.(false);
    if (order.join(',') !== latest.current.join(',')) onReorder(order);
    else {
      sync();
      if (!moved) onStill?.(id);
    }
  };

  return (
    <GestureHandlerRootView style={{ height }}>
      {items.map((item) => (
        <Row key={item.id} id={item.id} s={s} entering={isNew?.(item.id)} onMeasure={measured} onLift={lift} onDrop={drop}>
          {render(item)}
        </Row>
      ))}
    </GestureHandlerRootView>
  );
}

function Row({
  id,
  s,
  entering,
  onMeasure,
  onLift,
  onDrop,
  children,
}: {
  id: number;
  s: Shared;
  entering?: boolean;
  onMeasure: (id: number, h: number) => void;
  onLift: () => void;
  onDrop: (id: number, order: number[], moved: boolean) => void;
  children: ReactNode;
}) {
  const t = useTheme();
  const y = useSharedValue(UNPLACED);
  // Its slot: jumps there the first time (or while not measured), slides afterwards.
  useAnimatedReaction(
    () => [s.layout.value.top[id], s.layout.value.h[id]] as const,
    ([top, h]) => {
      if (top === undefined || s.active.value === id) return;
      y.value = y.value === UNPLACED || h === undefined ? top : withTiming(top, SLIDE);
    },
  );
  const pan = usePanGesture({
    activateAfterLongPress: HOLD,
    onActivate: (e) => {
      'worklet';
      s.startTop.value = y.value === UNPLACED ? (s.layout.value.top[id] ?? 0) : y.value;
      s.trans.value = 0;
      s.scrolled.value = 0;
      s.pointer.value = e.absoluteY;
      s.active.value = id;
      s.lifted.value = true;
      scheduleOnRN(onLift);
    },
    onUpdate: (e) => {
      'worklet';
      s.trans.value = e.translationY;
      s.pointer.value = e.absoluteY;
    },
    onDeactivate: (e) => {
      'worklet';
      if (!s.lifted.value || s.active.value !== id) return;
      s.lifted.value = false;
      const target = s.layout.value.top[id] ?? s.startTop.value;
      const moved = Math.abs(e.translationY) > 8;
      s.trans.value = withTiming(target - s.startTop.value - s.scrolled.value, SLIDE, () => {
        y.value = target;
        if (s.active.value === id) s.active.value = -1;
      });
      scheduleOnRN(onDrop, id, [...s.order.value], moved);
    },
  });
  const place = useAnimatedStyle(() => {
    const held = s.active.value === id;
    return {
      zIndex: held ? 10 : 0,
      transform: [{ translateY: held ? s.startTop.value + s.trans.value + s.scrolled.value : y.value }],
    };
  });
  const look = useAnimatedStyle(() => {
    const up = s.lifted.value && s.active.value === id;
    return {
      opacity: y.value === UNPLACED && s.active.value !== id ? 0 : 1,
      transform: [{ scale: withTiming(up ? 1.03 : 1, SLIDE) }],
    };
  });
  const shadow = useAnimatedStyle(() => ({ opacity: withTiming(s.lifted.value && s.active.value === id ? 1 : 0, SLIDE) }));
  return (
    <GestureDetector gesture={pan} touchAction="pan-y">
      <Animated.View
        entering={entering ? motion.enter : undefined}
        exiting={motion.exit}
        style={[{ position: 'absolute', left: 0, right: 0, top: 0 }, place]}>
        <Animated.View style={[NO_SELECT, look]} onLayout={(e) => onMeasure(id, Math.round(e.nativeEvent.layout.height))}>
          <Animated.View pointerEvents="none" style={[LIFT_SHADOW, { borderRadius: t.radius.card }, shadow]} />
          {children}
        </Animated.View>
      </Animated.View>
    </GestureDetector>
  );
}

const LIFT_SHADOW: ViewStyle = {
  position: 'absolute',
  left: 0,
  right: 0,
  top: 0,
  bottom: 0,
  boxShadow: '0 12px 28px rgba(0, 0, 0, 0.35)',
};
