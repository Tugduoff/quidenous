import Animated, { type SharedValue, useAnimatedProps, useSharedValue, withSpring } from 'react-native-reanimated';
import { Circle, G, Path } from 'react-native-svg';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';
import { BOUNCE } from '@/lib/motion';
import { type Rect, wirePath } from '@/lib/utils/choose';

const APath = Animated.createAnimatedComponent(Path);
const ACircle = Animated.createAnimatedComponent(Circle);

export type WireShared = {
  scrollY: SharedValue<number>;
  dragIdx: SharedValue<number>;
  dragX: SharedValue<number>;
  dragY: SharedValue<number>;
  hovered: SharedValue<number>;
  board: SharedValue<{ itemsX: number; rects: Rect[] }>;
};

export function Wire({
  color,
  ax,
  ay,
  scrollY,
  to,
  from,
}: {
  color: string;
  ax: number;
  ay: number;
  scrollY: SharedValue<number>;
  to: { x: number; y: number };
  from: { x: number; y: number };
}) {
  const ex = useSharedValue(from.x);
  const ey = useSharedValue(from.y);
  useAnimateOnChange(ex, [to.x, to.y], () => withSpring(to.x, BOUNCE));
  useAnimateOnChange(ey, [to.x, to.y], () => withSpring(to.y, BOUNCE));
  const path = useAnimatedProps(() => ({ d: wirePath(ax, ay - scrollY.value, ex.value, ey.value) }));
  const plug = useAnimatedProps(() => ({ cx: ex.value, cy: ey.value }));
  return (
    <G>
      <APath animatedProps={path} stroke="#000" strokeOpacity={0.22} strokeWidth={7} strokeLinecap="round" fill="none" />
      <APath animatedProps={path} stroke={color} strokeWidth={4.5} strokeLinecap="round" fill="none" />
      <ACircle animatedProps={plug} r={5.5} fill={color} stroke="#000" strokeOpacity={0.25} strokeWidth={1.5} />
    </G>
  );
}

export function DragWire({
  color,
  ax,
  step,
  first,
  shared: s,
}: {
  color: string;
  ax: number;
  step: number;
  first: number;
  shared: Pick<WireShared, 'scrollY' | 'dragIdx' | 'dragX' | 'dragY'>;
}) {
  const path = useAnimatedProps(() => {
    const i = s.dragIdx.value;
    if (i < 0) return { d: 'M -20 -20', opacity: 0 };
    return { d: wirePath(ax, first + i * step - s.scrollY.value, s.dragX.value, s.dragY.value), opacity: 1 };
  });
  const plug = useAnimatedProps(() => ({
    cx: s.dragX.value,
    cy: s.dragY.value,
    opacity: s.dragIdx.value < 0 ? 0 : 1,
  }));
  return (
    <G>
      <APath animatedProps={path} stroke="#000" strokeOpacity={0.22} strokeWidth={7} strokeLinecap="round" fill="none" />
      <APath animatedProps={path} stroke={color} strokeWidth={4.5} strokeLinecap="round" fill="none" />
      <ACircle animatedProps={plug} r={7} fill={color} stroke="#000" strokeOpacity={0.25} strokeWidth={1.5} />
    </G>
  );
}

export function FlyingWire({
  color,
  from,
  to,
  fly,
}: {
  color: string;
  from: { x: number; y: number };
  to: { x: number; y: number };
  fly: SharedValue<number>;
}) {
  const path = useAnimatedProps(() => {
    const x = from.x + (to.x - from.x) * fly.value;
    const y = from.y + (to.y - from.y) * fly.value;
    return { d: wirePath(from.x, from.y, x, y) };
  });
  const plug = useAnimatedProps(() => ({
    cx: from.x + (to.x - from.x) * fly.value,
    cy: from.y + (to.y - from.y) * fly.value,
  }));
  return (
    <G>
      <APath animatedProps={path} stroke="#000" strokeOpacity={0.22} strokeWidth={7} strokeLinecap="round" fill="none" />
      <APath animatedProps={path} stroke={color} strokeWidth={4.5} strokeLinecap="round" fill="none" />
      <ACircle animatedProps={plug} r={5.5} fill={color} />
    </G>
  );
}
