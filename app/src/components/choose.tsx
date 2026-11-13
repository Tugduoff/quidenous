import { useMutation, useQueryClient } from '@tanstack/react-query';
import * as Haptics from 'expo-haptics';
import { ArrowRight, Cable, Check, CircleCheck, Crown, List, Sparkles } from 'lucide-react-native';
import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react';
import { Platform, Pressable, Text, useWindowDimensions, View } from 'react-native';
import { GestureDetector, GestureHandlerRootView, usePanGesture } from 'react-native-gesture-handler';
import Animated, {
  FadeIn,
  ReduceMotion,
  type SharedValue,
  useAnimatedProps,
  useAnimatedScrollHandler,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
  ZoomIn,
} from 'react-native-reanimated';
import Svg, { Circle, G, Path } from 'react-native-svg';
import { scheduleOnRN } from 'react-native-worklets';

import { Screen } from '@/components/screen';
import { Tip } from '@/components/tip';
import { Avatar, AvatarStack, Button, Card, PressableScale } from '@/components/ui';
import { api, type Choice, type ChooseResult as ChooseData, type Member, type Round } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { BOUNCE, EASE, SPRING, usePop } from '@/lib/motion';
import { keys } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

// "Qui choisirait quoi ?" (docs/CHOOSE.md): wire every member to an item (mine: my real choice, the others: my guesses),
// then the result person by person.

const APath = Animated.createAnimatedComponent(Path);
const ACircle = Animated.createAnimatedComponent(Circle);
const sys = ReduceMotion.System;

/** A wire from a to b: a cubic that leaves and arrives flat, sagging a little like a real cable. */
function wirePath(ax: number, ay: number, bx: number, by: number) {
  'worklet';
  const dx = bx - ax;
  const sag = Math.min(26, 6 + Math.abs(dx) * 0.08 + Math.abs(by - ay) * 0.04);
  return `M ${ax} ${ay} C ${ax + dx * 0.45} ${ay + sag} ${bx - dx * 0.45} ${by + sag} ${bx} ${by}`;
}

/** "Léa Martin" → "Léa": names stay on one line in the narrow column. */
const firstName = (name: string) => name.trim().split(/\s+/)[0] || name;

const haptic = () => {
  if (Platform.OS !== 'web') Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
};

// ---------------------------------------------------------------------------------------------------------------------
// Ballot

type Geometry = {
  /** Row height, gap between rows, avatar size: compact as the group grows. */
  row: number;
  gap: number;
  avatar: number;
};

const PAD = 10;
/** Where a wire leaves a person's row: the socket at its right end (from the row's right edge). */
const SOCKET = 12;

function geometry(n: number): Geometry {
  if (n <= 6) return { row: 54, gap: 8, avatar: 34 };
  if (n <= 12) return { row: 46, gap: 6, avatar: 30 };
  return { row: 40, gap: 4, avatar: 26 };
}

/**
 * The ballot of a choose question: everyone in a column on the left, the items on the right. Drag a person to an item
 * (or tap a person, then an item); I'm first, my wire is my real choice, the others' are my guesses.
 */
export function ChooseBallot({
  round,
  members,
  byId,
  onVoted,
  top,
}: {
  round: Round;
  members: Member[];
  byId: Map<number, Member>;
  onVoted: () => void;
  /** Header and question, drawn by the Vote tab. */
  top: ReactNode;
}) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me } = useSession();
  const items = round.items ?? [];
  // Me first: the one real choice, then everyone I guess for.
  const people = useMemo(() => {
    const mine = members.find((m) => m.id === me?.id);
    return mine ? [mine, ...members.filter((m) => m.id !== mine.id)] : members;
  }, [members, me?.id]);
  const [choices, setChoices] = useState<Record<number, number>>({});
  const done = people.filter((p) => choices[p.id] != null).length;
  const all = people.length > 0 && done === people.length;
  const vote = useMutation({
    mutationFn: (c: Choice[]) => api('/round/current/vote', { method: 'POST', json: { choices: c } }),
    onSuccess: onVoted,
    onSettled: () => qc.invalidateQueries({ queryKey: keys.current }),
    // Someone joined or left meanwhile: refresh the room so the ballot shows who to wire now.
    onError: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  const send = () => all && vote.mutate(people.map((p) => ({ member_id: p.id, item: choices[p.id] })));
  const bump = usePop(all, 1.04, all);
  return (
    <Screen
      footer={
        <>
          {vote.isError && (
            <Text style={{ textAlign: 'center', fontFamily: t.font.semibold, fontSize: 13, color: t.tag }}>{tr('vote.choose.failed')}</Text>
          )}
          <Animated.View style={bump}>
            <Button testID="choose-vote" disabled={!all} loading={vote.isPending} onPress={send}>
              {all ? tr('vote.choose.submit') : tr('vote.choose.wired', { done, total: people.length })}
            </Button>
          </Animated.View>
        </>
      }>
      {top}
      {round.voter_ids.length > 0 && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} max={6} />
          <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
            {round.voter_ids.length}/{round.member_count}
          </Text>
        </View>
      )}
      <Tip id="choose" text={tr('vote.choose.tip')} />
      {items.length >= 2 && <WireBoard people={people} items={items} me={me?.id} choices={choices} onChange={setChoices} />}
    </Screen>
  );
}

type Rect = { y: number; h: number };

/** The board itself: people (scrolling inside it), items (always in view, centred), the wires between. */
function WireBoard({
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
  const g = geometry(people.length);
  const [width, setWidth] = useState(0);
  // Items: where their stack sits in the board, and each one in it.
  const [stackY, setStackY] = useState(0);
  const [stackH, setStackH] = useState(items.length * 60);
  const [rects, setRects] = useState<(Rect | undefined)[]>([]);

  const left = Math.round(width * 0.4);
  const itemsW = Math.round(width * 0.36);
  const itemsX = width - PAD - itemsW;
  const anchorX = PAD + left - SOCKET;
  const content = PAD * 2 + people.length * g.row + Math.max(0, people.length - 1) * g.gap;
  // Tall enough for the items; past a screenful of people, the list scrolls inside the board.
  const height = Math.max(stackH + PAD * 2, Math.min(content, Math.max(300, Math.round(win.height * 0.55))));
  const rowTop = (i: number) => PAD + i * (g.row + g.gap);

  const scrollY = useSharedValue(0);
  const onScroll = useAnimatedScrollHandler((e) => {
    scrollY.value = e.contentOffset.y;
  });
  // The wire being dragged: from row `dragIdx` (-1: none) to (dragX, dragY); `hovered` the item it would plug into.
  const dragIdx = useSharedValue(-1);
  const dragX = useSharedValue(0);
  const dragY = useSharedValue(0);
  const hovered = useSharedValue(-1);
  const board = useSharedValue<{ itemsX: number; rects: Rect[] }>({ itemsX: 0, rects: [] });
  const full = items.every((_, k) => !!rects[k]) ? items.map((_, k) => ({ y: stackY + rects[k]!.y, h: rects[k]!.h })) : null;
  useEffect(() => {
    board.value = { itemsX, rects: full ?? [] };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [itemsX, JSON.stringify(full)]);

  // Tap mode: the person waiting for an item. Dragging: the person whose wire is in hand (their own wire hides).
  const [sel, setSel] = useState<number | null>(null);
  const [dragging, setDragging] = useState<number | null>(null);
  // Kept after the drop: the wire springing back keeps its colour.
  const [dragColor, setDragColor] = useState(t.mut);
  // Each new plug remounts the wire, which springs from where it was let go (or from the person, tapped) into its socket.
  const plugs = useRef<Record<number, { n: number; x: number; y: number }>>({});
  const [bumps, setBumps] = useState<number[]>(() => items.map(() => 0));
  const lastDrag = useRef(0);

  const connect = (id: number, item: number, x: number, y: number) => {
    plugs.current[id] = { n: (plugs.current[id]?.n ?? 0) + 1, x, y };
    onChange({ ...choices, [id]: item });
    setBumps((b) => b.map((v, i) => (i === item ? v + 1 : v)));
    haptic();
  };
  // The live wire goes once the plugged one is on screen (same spot: no flicker).
  const plugged = useRef(false);
  useEffect(() => {
    if (!plugged.current) return;
    plugged.current = false;
    dragIdx.value = -1;
  }, [choices, dragIdx]);

  const onStart = (id: number) => {
    setSel(null);
    setDragging(id);
    setDragColor(people.find((p) => p.id === id)?.color ?? t.mut);
  };
  const onDrop = (id: number, x: number, y: number, item: number) => {
    lastDrag.current = Date.now();
    if (item >= 0) {
      plugged.current = true;
      connect(id, item, x, y);
    } else if (choices[id] != null) {
      // Let go anywhere else: unplugged.
      const next = { ...choices };
      delete next[id];
      onChange(next);
    }
    setDragging(null);
  };
  const tapPerson = (id: number) => {
    // The end of a drag on the same row is not a tap.
    if (Date.now() - lastDrag.current < 350) return;
    setSel((s) => (s === id ? null : id));
  };
  const tapItem = (item: number) => {
    if (sel == null) return;
    const i = people.findIndex((p) => p.id === sel);
    connect(sel, item, anchorX, rowTop(i) + g.row / 2 - scrollY.value);
    // On to the next person still to wire, in order.
    const rest = [...people.slice(i + 1), ...people.slice(0, i)].find((p) => p.id !== sel && choices[p.id] == null);
    setSel(rest?.id ?? null);
  };

  // Several wires into one item: their plugs spread along its edge.
  const ends: Record<number, { x: number; y: number }> = {};
  full?.forEach((r, k) => {
    const into = people.filter((p) => choices[p.id] === k);
    into.forEach((p, j) => {
      const y = into.length === 1 ? r.y + r.h / 2 : r.y + 10 + ((r.h - 20) * j) / (into.length - 1);
      ends[p.id] = { x: itemsX - 2, y };
    });
  });

  const selected = people.find((p) => p.id === sel);
  return (
    <GestureHandlerRootView style={{ height }}>
      <View
        onLayout={(e) => setWidth(e.nativeEvent.layout.width)}
        style={[{ height, borderRadius: t.radius.card, backgroundColor: t.card, overflow: 'hidden' }, t.shadow]}>
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
                    const plug = plugs.current[p.id];
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
              <View
                style={{ gap: 10 }}
                onLayout={(e) => {
                  setStackY(e.nativeEvent.layout.y);
                  setStackH(e.nativeEvent.layout.height);
                }}>
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

type Shared = {
  scrollY: SharedValue<number>;
  dragIdx: SharedValue<number>;
  dragX: SharedValue<number>;
  dragY: SharedValue<number>;
  hovered: SharedValue<number>;
  board: SharedValue<{ itemsX: number; rects: Rect[] }>;
};

/** The item a wire let go at (x, y) plugs into: once near the items' column, the closest one. -1: none. */
function hit(x: number, y: number, b: { itemsX: number; rects: Rect[] }) {
  'worklet';
  if (x < b.itemsX - 48 || !b.rects.length) return -1;
  let best = -1;
  let dist = Infinity;
  for (let k = 0; k < b.rects.length; k++) {
    const d = Math.abs(y - (b.rects[k].y + b.rects[k].h / 2));
    if (d < dist) {
      dist = d;
      best = k;
    }
  }
  return best;
}

/** Someone to wire: their face, their name (or "Toi"), the socket the wire leaves from. Drag it to an item, or tap it. */
function PersonRow({
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
  shared: Shared;
  onStart: (id: number) => void;
  onDrop: (id: number, x: number, y: number, item: number) => void;
  onPress: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const id = m.id;
  const pan = usePanGesture({
    // Rightwards only: up and down still scrolls the list.
    activeOffsetX: 10,
    failOffsetY: [-20, 20],
    onActivate: (e) => {
      'worklet';
      s.dragIdx.value = index;
      s.dragX.value = PAD + e.x;
      s.dragY.value = top - s.scrollY.value + e.y;
      scheduleOnRN(onStart, id);
    },
    onUpdate: (e) => {
      'worklet';
      const x = PAD + e.x;
      const y = top - s.scrollY.value + e.y;
      s.dragX.value = x;
      s.dragY.value = y;
      s.hovered.value = hit(x, y, s.board.value);
    },
    onDeactivate: () => {
      'worklet';
      const k = s.hovered.value;
      s.hovered.value = -1;
      if (k >= 0) {
        scheduleOnRN(onDrop, id, s.dragX.value, s.dragY.value, k);
        return;
      }
      // Nowhere to plug in: the wire springs back into its socket.
      s.dragX.value = withSpring(anchor.x, SPRING);
      s.dragY.value = withSpring(anchor.y - s.scrollY.value, SPRING, (finished) => {
        if (finished && s.dragIdx.value === index) s.dragIdx.value = -1;
      });
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
            {/* The one real choice; the other rows are guesses. */}
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

/** An item to plug into: grows a little while a wire hovers it, bumps when one snaps in. */
function ItemPlug({
  index,
  label,
  hovered,
  bump,
  armed,
  a11y,
  onLayout,
  onPress,
}: {
  index: number;
  label: string;
  hovered: SharedValue<number>;
  bump: number;
  armed: boolean;
  a11y: string;
  onLayout: (r: Rect) => void;
  onPress: () => void;
}) {
  const t = useTheme();
  const over = useAnimatedStyle(() => ({ transform: [{ scale: withSpring(hovered.value === index ? 1.06 : 1, SPRING) }] }));
  const snap = usePop(bump, 1.06, bump > 0);
  return (
    <Animated.View onLayout={(e) => onLayout({ y: e.nativeEvent.layout.y, h: e.nativeEvent.layout.height })} style={over}>
      <Animated.View style={snap}>
        <PressableScale
          testID={`choose-item-${index}`}
          accessibilityRole="button"
          accessibilityLabel={a11y}
          onPress={onPress}
          style={{
            minHeight: 52,
            justifyContent: 'center',
            paddingVertical: 10,
            paddingHorizontal: 12,
            borderRadius: 16,
            borderWidth: 2,
            borderColor: armed ? t.mut : t.line,
            backgroundColor: t.card2,
          }}>
          <Text numberOfLines={3} style={{ fontFamily: t.font.bold, fontSize: 15, lineHeight: 19, color: t.txt }}>
            {label}
          </Text>
        </PressableScale>
      </Animated.View>
    </Animated.View>
  );
}

/** A plugged wire: follows its person as the list scrolls; its plug springs into the socket. */
function Wire({
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
  useEffect(() => {
    ex.value = withSpring(to.x, BOUNCE);
    ey.value = withSpring(to.y, BOUNCE);
  }, [to.x, to.y, ex, ey]);
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

/** The wire in hand, from its person to the finger (or the mouse). Nothing drawn when no drag is on. */
function DragWire({
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
  shared: Pick<Shared, 'scrollY' | 'dragIdx' | 'dragX' | 'dragY'>;
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

// ---------------------------------------------------------------------------------------------------------------------
// Waiting

/** My own real choice, once I voted (null: an older server, or not found). */
export function myRealChoice(round: Round, me: number | undefined): string | null {
  const c = round.my_choices?.find((x) => x.member_id === me);
  return c ? (round.items?.[c.item] ?? null) : null;
}

// ---------------------------------------------------------------------------------------------------------------------
// Result

/** A finished choose question: one person at a time (or all at once), what they thought of me, who guessed best. */
export function ChooseResult({ round, data, byId }: { round: Round; data: ChooseData; byId: Map<number, Member> }) {
  const items = round.items ?? [];
  const { me } = useSession();
  const people = data.people.filter((p) => byId.has(p.member_id));
  const [all, setAll] = useState(false);
  const mine = people.find((p) => p.member_id === me?.id);
  return (
    <View style={{ gap: 20 }}>
      {all ? (
        <Summary people={people} items={items} byId={byId} onOne={() => setAll(false)} />
      ) : (
        <Reveal people={people} items={items} byId={byId} onAll={() => setAll(true)} />
      )}
      {mine && mine.guesses.length > 0 && <AboutMe person={mine} items={items} byId={byId} />}
      <BestGuessers scores={data.scores} byId={byId} />
    </View>
  );
}

type Person = ChooseData['people'][number];

/** How a person's real choice compares with the group's guess. */
function verdict(p: Person): 'match' | 'surprise' | 'noGuess' | 'noAnswer' {
  if (p.real == null) return 'noAnswer';
  if (!p.top.length) return 'noGuess';
  return p.top.includes(p.real) ? 'match' : 'surprise';
}

function Verdict({ p, compact }: { p: Person; compact?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const v = verdict(p);
  const saw = p.real == null ? 0 : p.guesses.filter((g) => g.item === p.real).length;
  const [Icon, color] = v === 'match' ? [CircleCheck, t.accInk] : v === 'surprise' ? [Sparkles, t.tag] : [null, t.mut];
  if (compact) return Icon ? <Icon size={18} color={color} /> : null;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 6 }}>
      {Icon && <Icon size={16} color={color} />}
      <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color }}>
        {tr(`results.choose.${v}`)}
      </Text>
      {p.guesses.length > 0 && p.real != null && (
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {` · ${tr('results.choose.saw', { n: saw, total: p.guesses.length })}`}
        </Text>
      )}
    </View>
  );
}

const WIRE_MS = 520;

/** One person at a time: the wire flies to what the group guessed, then their real choice lights up. */
function Reveal({ people, items, byId, onAll }: { people: Person[]; items: string[]; byId: Map<number, Member>; onAll: () => void }) {
  const t = useTheme();
  const tr = useT();
  const [step, setStep] = useState(0);
  const p = people[Math.min(step, people.length - 1)];
  if (!p) return null;
  const last = step >= people.length - 1;
  return (
    <Card style={{ padding: 16, gap: 14 }}>
      <RevealStage key={p.member_id} p={p} items={items} byId={byId} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
          {step + 1}/{people.length}
        </Text>
        <View style={{ flex: 1 }} />
        {!last && (
          <Pressable accessibilityRole="button" onPress={onAll} hitSlop={8} style={{ minHeight: 40, justifyContent: 'center', paddingHorizontal: 6 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('results.choose.all')}</Text>
          </Pressable>
        )}
        <PressableScale
          accessibilityRole="button"
          onPress={() => (last ? onAll() : setStep(step + 1))}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 6,
            height: 40,
            paddingHorizontal: 16,
            borderRadius: t.radius.pill,
            backgroundColor: t.card2,
          }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{tr(last ? 'results.choose.all' : 'results.choose.next')}</Text>
          {last ? <List size={16} color={t.txt} /> : <ArrowRight size={16} color={t.txt} />}
        </PressableScale>
      </View>
    </Card>
  );
}

type Box = { x: number; y: number; w: number; h: number };
const boxOf = (l: { x: number; y: number; width: number; height: number }): Box => ({ x: l.x, y: l.y, w: l.width, h: l.height });

function RevealStage({ p, items, byId }: { p: Person; items: string[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const m = byId.get(p.member_id);
  const color = m?.color ?? t.mut;
  const [size, setSize] = useState({ w: 0, h: 0 });
  // The face inside its column (name under it), the column inside the row.
  const [face, setFace] = useState<Box | null>(null);
  const [who, setWho] = useState<Box | null>(null);
  const [col, setCol] = useState<Box | null>(null);
  const [rows, setRows] = useState<(Box | undefined)[]>([]);
  const ready = !!face && !!who && !!col && items.every((_, k) => !!rows[k]);
  // The wire(s) fly out once everything is measured; the real choice lights up after they land.
  const fly = useSharedValue(0);
  useEffect(() => {
    if (ready) fly.value = withTiming(1, { duration: WIRE_MS, easing: EASE, reduceMotion: sys });
  }, [ready, fly]);
  const from = face && who ? { x: who.x + face.x + face.w + 2, y: who.y + face.y + face.h / 2 } : { x: 0, y: 0 };
  return (
    <View style={{ gap: 14 }}>
      <View
        onLayout={(e) => setSize({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 48 }}>
        <View style={{ width: 76, alignItems: 'center', gap: 6 }} onLayout={(e) => setWho(boxOf(e.nativeEvent.layout))}>
          <View onLayout={(e) => setFace(boxOf(e.nativeEvent.layout))}>
            <Avatar member={m} size={56} />
          </View>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
            {m ? firstName(m.name) : '?'}
          </Text>
        </View>
        <View style={{ flex: 1, gap: 8 }} onLayout={(e) => setCol(boxOf(e.nativeEvent.layout))}>
          {items.map((label, k) => {
            const voters = p.guesses.filter((g) => g.item === k).map((g) => byId.get(g.voter_id));
            const real = p.real === k;
            return (
              <View
                key={k}
                onLayout={(e) => {
                  const b = boxOf(e.nativeEvent.layout);
                  setRows((rs) => Object.assign([...rs], { [k]: b }));
                }}
                style={{
                  minHeight: 44,
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 8,
                  paddingVertical: 8,
                  paddingHorizontal: 10,
                  borderRadius: 14,
                  backgroundColor: t.card2,
                }}>
                <Text numberOfLines={2} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 14, lineHeight: 18, color: t.txt }}>
                  {label}
                </Text>
                {voters.length > 0 && <AvatarStack members={voters} size={18} ring={t.card2} max={3} />}
                {/* Their own wire. */}
                {real && (
                  <Animated.View
                    entering={FadeIn.delay(WIRE_MS + 120)
                      .duration(220)
                      .reduceMotion(sys)}
                    pointerEvents="none"
                    style={{ position: 'absolute', left: -2, top: -2, right: -2, bottom: -2, borderRadius: 16, borderWidth: 2, borderColor: color }}
                  />
                )}
                {real && (
                  <Animated.View
                    entering={ZoomIn.delay(WIRE_MS + 120)
                      .springify()
                      .damping(12)
                      .reduceMotion(sys)}
                    style={{ width: 20, height: 20, borderRadius: 10, alignItems: 'center', justifyContent: 'center', backgroundColor: color }}>
                    <Check size={13} color="#121216" strokeWidth={3} />
                  </Animated.View>
                )}
              </View>
            );
          })}
        </View>
        {ready && size.w > 0 && (
          <View pointerEvents="none" style={{ position: 'absolute', left: 0, top: 0, width: size.w, height: size.h }}>
            <Svg width={size.w} height={size.h}>
              {p.top.map((k) => {
                const r = rows[k]!;
                return <FlyingWire key={k} color={color} from={from} to={{ x: col!.x - 2, y: col!.y + r.y + r.h / 2 }} fly={fly} />;
              })}
            </Svg>
          </View>
        )}
      </View>
      <Animated.View
        entering={FadeIn.delay(WIRE_MS + 220)
          .duration(220)
          .reduceMotion(sys)}>
        <Verdict p={p} />
      </Animated.View>
    </View>
  );
}

/** The group's guess: a wire that shoots from the person to the item, plug first. */
function FlyingWire({
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

/** Everyone at once: the group's guess against the real choice. */
function Summary({ people, items, byId, onOne }: { people: Person[]; items: string[]; byId: Map<number, Member>; onOne: () => void }) {
  const t = useTheme();
  const tr = useT();
  const name = (k: number | null | undefined) => (k == null ? '?' : (items[k] ?? '?'));
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      {people.map((p, i) => {
        const m = byId.get(p.member_id);
        const v = verdict(p);
        const guess = p.top.map((k) => items[k]).join(' / ');
        return (
          <Animated.View
            key={p.member_id}
            entering={FadeIn.delay(Math.min(i, 8) * 40)
              .duration(200)
              .reduceMotion(sys)}
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
            <Avatar member={m} size={32} />
            <View style={{ flex: 1, minWidth: 0 }}>
              <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
                {m?.name ?? '?'}
              </Text>
              <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
                {v === 'surprise' ? (
                  <>
                    {`${guess} → `}
                    <Text style={{ fontFamily: t.font.semibold, color: t.txt }}>{name(p.real)}</Text>
                  </>
                ) : v === 'match' ? (
                  <Text style={{ fontFamily: t.font.semibold, color: t.txt }}>{name(p.real)}</Text>
                ) : v === 'noGuess' ? (
                  <>
                    <Text style={{ fontFamily: t.font.semibold, color: t.txt }}>{name(p.real)}</Text>
                    {` · ${tr('results.choose.noGuess')}`}
                  </>
                ) : (
                  tr('results.choose.noAnswer')
                )}
              </Text>
            </View>
            <Verdict p={p} compact />
          </Animated.View>
        );
      })}
      <Pressable
        accessibilityRole="button"
        onPress={onOne}
        hitSlop={8}
        style={{ alignSelf: 'center', minHeight: 40, justifyContent: 'center', paddingHorizontal: 12 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('results.choose.oneByOne')}</Text>
      </Pressable>
    </Card>
  );
}

function SectionTitle({ children }: { children: string }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>{children}</Text>;
}

/** What each of them guessed for me, by item; my real one ringed in my colour. */
function AboutMe({ person: p, items, byId }: { person: Person; items: string[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const color = byId.get(p.member_id)?.color ?? t.mut;
  const rows = items.map((label, k) => ({ label, k, voters: p.guesses.filter((g) => g.item === k).map((g) => byId.get(g.voter_id)) }));
  return (
    <View style={{ gap: 10 }}>
      <SectionTitle>{tr('results.choose.aboutMe')}</SectionTitle>
      <Card style={{ padding: 12, gap: 8 }}>
        {rows
          .filter((r) => r.voters.length > 0 || r.k === p.real)
          .map((r) => {
            const real = r.k === p.real;
            return (
              <View
                key={r.k}
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 10,
                  minHeight: 44,
                  paddingHorizontal: 10,
                  borderRadius: 14,
                  borderWidth: 2,
                  borderColor: real ? color : 'transparent',
                  backgroundColor: t.card2,
                }}>
                <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {r.label}
                </Text>
                {r.voters.length > 0 ? (
                  <AvatarStack members={r.voters} size={24} ring={t.card2} max={5} />
                ) : (
                  <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('results.choose.nobody')}</Text>
                )}
                {real && <Check size={16} color={color} strokeWidth={3} />}
              </View>
            );
          })}
      </Card>
    </View>
  );
}

/** Who guessed best: hits out of the guesses that could hit. */
function BestGuessers({ scores, byId }: { scores: ChooseData['scores']; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const [more, setMore] = useState(false);
  const list = scores.filter((s) => s.total > 0 && byId.has(s.member_id));
  if (!list.length) return null;
  const shown = more || list.length <= 6 ? list : list.slice(0, 5);
  const best = list[0].hits;
  return (
    <View style={{ gap: 10 }}>
      <SectionTitle>{tr('results.choose.best')}</SectionTitle>
      <Card style={{ padding: 12, gap: 6 }}>
        {shown.map((s, i) => {
          const m = byId.get(s.member_id);
          return (
            <View key={s.member_id} style={{ flexDirection: 'row', alignItems: 'center', gap: 10, minHeight: 40 }}>
              <Text style={{ width: 20, textAlign: 'center', fontFamily: t.font.display, fontSize: 15, color: t.mut }}>{i + 1}</Text>
              <Avatar member={m} size={30} />
              <View style={{ flex: 1, minWidth: 0, flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {m?.name ?? '?'}
                </Text>
                {best > 0 && s.hits === best && <Crown size={15} color={m?.color ?? t.mut} />}
              </View>
              <Text style={{ fontFamily: t.font.display, fontSize: 17, color: t.txt }}>
                {s.hits}
                <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>/{s.total}</Text>
              </Text>
            </View>
          );
        })}
        {shown.length < list.length && (
          <Pressable
            accessibilityRole="button"
            onPress={() => setMore(true)}
            hitSlop={8}
            style={{ alignSelf: 'center', minHeight: 40, justifyContent: 'center' }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('results.seeAll', { count: list.length })}</Text>
          </Pressable>
        )}
      </Card>
    </View>
  );
}

/** A choose question's icon (wires), for lists and tags. */
export const ChooseIcon = Cable;

/** Items in one muted line: "Pizza · Sushi · Burger". */
export const itemsLine = (items: string[] | null | undefined) => (items ?? []).join(' · ');
