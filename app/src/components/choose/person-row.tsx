import { Text, View } from 'react-native';
import { GestureDetector, usePanGesture } from 'react-native-gesture-handler';
import Animated, { withSpring } from 'react-native-reanimated';
import { scheduleOnRN } from 'react-native-worklets';
import type { WireShared } from '@/components/choose/wires';
import { Avatar } from '@/components/ui/avatar';
import { PressableScale } from '@/components/ui/pressable-scale';
import type { Member } from '@/lib/api/group';
import { useT } from '@/lib/i18n';
import { SPRING } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { type Geometry, hit, PAD, SOCKET } from '@/lib/utils/choose';
import { firstName } from '@/lib/utils/names';

export function PersonRow({
  member: m,
  index,
  me,
  g,
  width,
  wired,
  selected,
  top,
  anchor,
  shared: s,
  onStart,
  onDrop,
  onPress,
}: {
  member: Member;
  index: number;
  me: boolean;
  g: Geometry;
  width: number;
  wired: string | undefined;
  selected: boolean;
  top: number;
  anchor: { x: number; y: number };
  shared: WireShared;
  onStart: (id: number) => void;
  onDrop: (id: number, x: number, y: number, item: number) => void;
  onPress: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const id = m.id;
  const pan = usePanGesture({
    activeOffsetX: 10,
    failOffsetY: [-20, 20],
    onActivate: (e) => {
      'worklet';
      s.dragIdx.set(index);
      s.dragX.set(PAD + e.x);
      s.dragY.set(top - s.scrollY.value + e.y);
      scheduleOnRN(onStart, id);
    },
    onUpdate: (e) => {
      'worklet';
      const x = PAD + e.x;
      const y = top - s.scrollY.value + e.y;
      s.dragX.set(x);
      s.dragY.set(y);
      s.hovered.set(hit(x, y, s.board.value));
    },
    onDeactivate: () => {
      'worklet';
      const k = s.hovered.value;
      s.hovered.set(-1);
      if (k >= 0) {
        scheduleOnRN(onDrop, id, s.dragX.value, s.dragY.value, k);
        return;
      }
      s.dragX.set(withSpring(anchor.x, SPRING));
      s.dragY.set(
        withSpring(anchor.y - s.scrollY.value, SPRING, (finished) => {
          if (finished && s.dragIdx.value === index) s.dragIdx.set(-1);
        }),
      );
      scheduleOnRN(onDrop, id, 0, 0, -1);
    },
  });
  const label = me ? tr('vote.choose.me') : m.name;
  return (
    <GestureDetector gesture={pan} touchAction="pan-y">
      <Animated.View style={{ width }}>
        <PressableScale
          testID={`choose-person-${id}`}
          scale={0.97}
          accessibilityRole="button"
          accessibilityState={{ selected }}
          accessibilityLabel={wired ? tr('vote.choose.wiredTo', { name: label, item: wired }) : tr('vote.choose.pick', { name: label })}
          onPress={() => onPress(id)}
          style={{
            height: g.row,
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            paddingLeft: 6,
            paddingRight: 6,
            borderRadius: g.row / 2,
            borderWidth: 2,
            borderColor: selected ? m.color : 'transparent',
            backgroundColor: selected ? t.card2 : 'transparent',
          }}>
          <Avatar member={m} size={g.avatar} />
          <View style={{ flex: 1, minWidth: 0 }}>
            <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: g.row > 42 ? 15 : 14, color: t.txt }}>
              {me ? label : firstName(m.name)}
            </Text>
            {me && (
              <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 11, color: t.mut }}>
                {tr('vote.choose.real')}
              </Text>
            )}
          </View>
          <View
            style={{
              width: SOCKET,
              height: SOCKET,
              borderRadius: SOCKET / 2,
              borderWidth: 2,
              borderColor: m.color,
              backgroundColor: wired ? m.color : t.card,
            }}
          />
        </PressableScale>
      </Animated.View>
    </GestureDetector>
  );
}
