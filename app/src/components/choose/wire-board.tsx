import { type ComponentRef, useRef, useState } from 'react';
import { Platform, useWindowDimensions, View } from 'react-native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import Animated, { useAnimatedScrollHandler, useSharedValue } from 'react-native-reanimated';
import Svg from 'react-native-svg';
import { ItemPlug } from '@/components/choose/item-plug';
import { PersonRow } from '@/components/choose/person-row';
import { DragWire, Wire } from '@/components/choose/wires';
import type { Member } from '@/lib/api/group';
import { impactHaptic } from '@/lib/device/haptics';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';
import { useOnChange } from '@/lib/hooks/use-on-change';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { boardLayout, measured, nextToWire, PAD, plugEnds, type Rect, rowTop as rowTopOf } from '@/lib/utils/choose';

const NO_SELECT = Platform.OS === 'web' ? ({ userSelect: 'none' } as object) : null;
const BOTTOM = 150;

export function WireBoard({
  people,
  items,
  me,
  choices,
  onChange,
}: {
  people: Member[];
  items: string[];
  me: number | undefined;
  choices: Record<number, number>;
  onChange: (c: Record<number, number>) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const win = useWindowDimensions();
  const [width, setWidth] = useState(0);
  const [stackH, setStackH] = useState(items.length * 60);
  const [rects, setRects] = useState<(Rect | undefined)[]>([]);

  const box = useRef<ComponentRef<typeof View>>(null);
  const [boardTop, setBoardTop] = useState<number | null>(null);
  const room = boardTop == null ? Math.round(win.height * 0.55) : win.height - boardTop - BOTTOM;
  const { g, itemsW, itemsX, left, anchorX, content, height, stackY } = boardLayout(width, people.length, items, stackH, room);
  const rowTop = (i: number) => rowTopOf(g, i);

  const scrollY = useSharedValue(0);
  const onScroll = useAnimatedScrollHandler((e) => {
    scrollY.set(e.contentOffset.y);
  });
  const dragIdx = useSharedValue(-1);
  const dragX = useSharedValue(0);
  const dragY = useSharedValue(0);
  const hovered = useSharedValue(-1);
  const board = useSharedValue<{ itemsX: number; rects: Rect[] }>({ itemsX: 0, rects: [] });
  const full = measured(rects, items.length)?.map((r) => ({ y: stackY + r.y, h: r.h })) ?? null;
  const fullKey = JSON.stringify(full);
  useAnimateOnChange(board, [itemsX, fullKey], () => ({ itemsX, rects: full ?? [] }));

  const [sel, setSel] = useState<number | null>(null);
  const [dragging, setDragging] = useState<number | null>(null);
  const [dragColor, setDragColor] = useState(t.mut);
  const [plugs, setPlugs] = useState<Record<number, { n: number; x: number; y: number }>>({});
  const [bumps, setBumps] = useState<number[]>(() => items.map(() => 0));
  const justDropped = useRef(false);

  const connect = (id: number, item: number, x: number, y: number) => {
    setPlugs((all) => ({ ...all, [id]: { n: (all[id]?.n ?? 0) + 1, x, y } }));
    onChange({ ...choices, [id]: item });
    setBumps((b) => b.map((v, i) => (i === item ? v + 1 : v)));
    impactHaptic();
  };
  const plugged = useRef(false);
  useOnChange(choices, () => {
    if (!plugged.current) return;
    plugged.current = false;
    dragIdx.set(-1);
  });

  const onStart = (id: number) => {
    setSel(null);
    setDragging(id);
    setDragColor(people.find((p) => p.id === id)?.color ?? t.mut);
  };
  const onDrop = (id: number, x: number, y: number, item: number) => {
    justDropped.current = true;
    setTimeout(() => {
      justDropped.current = false;
    }, 350);
    if (item >= 0) {
      plugged.current = true;
      connect(id, item, x, y);
    } else if (choices[id] != null) {
      const next = { ...choices };
      delete next[id];
      onChange(next);
    }
    setDragging(null);
  };
  const tapPerson = (id: number) => {
    if (justDropped.current) return;
    setSel((s) => (s === id ? null : id));
  };
  const tapItem = (item: number) => {
    if (sel == null) return;
    const i = people.findIndex((p) => p.id === sel);
    connect(sel, item, anchorX, rowTop(i) + g.row / 2 - scrollY.value);
    const rest = nextToWire(people, i, sel, choices);
    setSel(rest?.id ?? null);
  };

  const ends = full ? plugEnds(full, people, choices, itemsX - 2) : {};

  const selected = people.find((p) => p.id === sel);
  return (
    <GestureHandlerRootView style={{ height }}>
      <View
        ref={box}
        onLayout={(e) => {
          setWidth(e.nativeEvent.layout.width);
          box.current?.measureInWindow((_x: number, y: number) => setBoardTop((old) => (old == null || Math.abs(old - y) > 4 ? y : old)));
        }}
        style={[{ height, borderRadius: t.radius.card, backgroundColor: t.card, overflow: 'hidden' }, NO_SELECT, t.shadow]}>
        {width > 0 && (
          <>
            <Animated.ScrollView
              onScroll={onScroll}
              scrollEventThrottle={16}
              nestedScrollEnabled
              showsVerticalScrollIndicator={content > height}
              contentContainerStyle={{ padding: PAD, gap: g.gap }}>
              {people.map((p, i) => (
                <PersonRow
                  key={p.id}
                  member={p}
                  index={i}
                  me={p.id === me}
                  g={g}
                  width={left}
                  wired={choices[p.id] != null ? items[choices[p.id]] : undefined}
                  selected={sel === p.id}
                  top={rowTop(i)}
                  anchor={{ x: anchorX, y: rowTop(i) + g.row / 2 }}
                  shared={{ scrollY, dragIdx, dragX, dragY, hovered, board }}
                  onStart={onStart}
                  onDrop={onDrop}
                  onPress={tapPerson}
                />
              ))}
            </Animated.ScrollView>
            <View pointerEvents="none" style={{ position: 'absolute', left: 0, top: 0, width, height }}>
              <Svg width={width} height={height}>
                {full &&
                  people.map((p, i) => {
                    const end = ends[p.id];
                    if (!end || dragging === p.id) return null;
                    const plug = plugs[p.id];
                    return (
                      <Wire
                        key={`${p.id}-${plug?.n ?? 0}`}
                        color={p.color}
                        ax={anchorX}
                        ay={rowTop(i) + g.row / 2}
                        scrollY={scrollY}
                        to={end}
                        from={plug ?? end}
                      />
                    );
                  })}
                <DragWire color={dragColor} ax={anchorX} step={g.row + g.gap} first={PAD + g.row / 2} shared={{ scrollY, dragIdx, dragX, dragY }} />
              </Svg>
            </View>
            <View style={{ position: 'absolute', top: 0, bottom: 0, right: PAD, width: itemsW, justifyContent: 'center' }}>
              <View style={{ gap: 10 }} onLayout={(e) => setStackH(e.nativeEvent.layout.height)}>
                {items.map((label, k) => (
                  <ItemPlug
                    key={k}
                    index={k}
                    label={label}
                    hovered={hovered}
                    bump={bumps[k] ?? 0}
                    armed={!!selected}
                    a11y={selected ? tr('vote.choose.wireTo', { name: selected.name, item: label }) : label}
                    onLayout={(r) => setRects((rs) => Object.assign([...rs], { [k]: r }))}
                    onPress={() => tapItem(k)}
                  />
                ))}
              </View>
            </View>
          </>
        )}
      </View>
    </GestureHandlerRootView>
  );
}
