// Live reactions on a result (docs/REACTIONS.md): the bar of 8 emojis with their counts, the bursts that float up
// when someone taps one, and the big screen's version of both.
import * as Haptics from 'expo-haptics';
import { memo, useCallback, useRef, useState } from 'react';
import { Platform, ScrollView, StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import Animated, { Easing, FadeIn, makeMutable, ReduceMotion, type SharedValue, useAnimatedStyle, useReducedMotion, withDelay, withTiming } from 'react-native-reanimated';
import { scheduleOnUI } from 'react-native-worklets';
import type { Member } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { formatNumber, useT } from '@/lib/i18n';
import { useBlocks } from '@/lib/moderation';
import { usePop } from '@/lib/motion';
import { burstList, emojiIndex, type LiveTap, RESULT_CAP, RESULT_REACTIONS, type ResultReaction, useReactedListener, useResultReactions } from '@/lib/result-reactions';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Dialog } from '@/components/ui/dialog';

// The burst pool: a fixed set of slots, each a little window onto a strip of the 8 emojis. A burst only writes shared
// values (which emoji, where, how it drifts, then its progress 0 → 1) and the UI thread does the rest: no React render,
// however many taps come in. A new burst takes the oldest slot (a ring).

type Burst = {
  x: number;
  y: number;
  /** Which emoji of the palette (the strip's offset). */
  idx: number;
  rise: number;
  drift: number;
  wobble: number;
  freq: number;
  phase: number;
  scale: number;
  tilt: number;
};
type Slot = { p: SharedValue<number>; b: SharedValue<Burst> };
const IDLE: Burst = { x: 0, y: 0, idx: 0, rise: 0, drift: 0, wobble: 0, freq: 1, phase: 0, scale: 1, tilt: 0 };

/** How bursts of one pool move: ranges picked at random per burst. */
type BurstStyle = { rise: [number, number]; drift: number; wobble: number; duration: [number, number]; scale: [number, number] };
const PHONE: BurstStyle = { rise: [220, 320], drift: 34, wobble: 12, duration: [1100, 1500], scale: [0.85, 1.15] };

const between = ([a, b]: [number, number]) => a + Math.random() * (b - a);

function start(p: SharedValue<number>, b: SharedValue<Burst>, burst: Burst, delay: number, duration: number) {
  'worklet';
  b.value = burst;
  p.value = 0;
  p.value = withDelay(delay, withTiming(1, { duration, easing: Easing.linear }));
}

/**
 * A pool of `size` burst slots, and `fire(emoji, x, y)` to play one from there (in the layer's coordinates). Nothing
 * under reduced motion.
 */
export function useBurstPool(size: number, style: BurstStyle = PHONE) {
  const reduced = useReducedMotion();
  const [slots] = useState<Slot[]>(() => Array.from({ length: size }, () => ({ p: makeMutable(0), b: makeMutable(IDLE) })));
  const cursor = useRef(0);
  const fire = useCallback(
    (emoji: string, x: number, y: number, delay = 0, rise?: [number, number]) => {
      const idx = emojiIndex(emoji);
      if (reduced || idx < 0) return;
      const slot = slots[cursor.current];
      cursor.current = (cursor.current + 1) % slots.length;
      const burst: Burst = {
        x,
        y,
        idx,
        rise: between(rise ?? style.rise),
        drift: (Math.random() * 2 - 1) * style.drift,
        wobble: (Math.random() * 0.6 + 0.4) * style.wobble * (Math.random() < 0.5 ? -1 : 1),
        freq: 1 + Math.random() * 1.2,
        phase: Math.random() * Math.PI * 2,
        scale: between(style.scale),
        tilt: (Math.random() * 2 - 1) * 14,
      };
      scheduleOnUI(start, slot.p, slot.b, burst, delay, between(style.duration));
    },
    [reduced, slots, style],
  );
  return { slots, fire };
}

/** The slots of a pool, drawn over whatever is under them (never touchable, never read out). */
export function BurstLayer({ slots, size, clip }: { slots: Slot[]; size: number; clip?: boolean }) {
  return (
    <View aria-hidden pointerEvents="none" style={[StyleSheet.absoluteFill, clip && { overflow: 'hidden' }]}>
      {slots.map((slot, i) => (
        <BurstSlot key={i} slot={slot} size={size} />
      ))}
    </View>
  );
}

const BurstSlot = memo(function BurstSlot({ slot, size }: { slot: Slot; size: number }) {
  const w = Math.round(size * 1.5);
  const h = Math.round(size * 1.35);
  const outer = useAnimatedStyle(() => {
    const p = slot.p.value;
    const b = slot.b.value;
    const on = p > 0 && p < 1;
    // Rises fast then slows, sways a little, pops in, shrinks a bit, fades in its last third.
    const up = 1 - (1 - p) * (1 - p);
    const dx = b.drift * p + b.wobble * Math.sin(p * b.freq * Math.PI * 2 + b.phase);
    const s = p < 0.12 ? 0.35 + 0.85 * (p / 0.12) : p < 0.26 ? 1.2 - 0.2 * ((p - 0.12) / 0.14) : 1 - 0.18 * ((p - 0.26) / 0.74);
    const opacity = !on ? 0 : p < 0.06 ? p / 0.06 : p > 0.62 ? (1 - p) / 0.38 : 1;
    return {
      opacity,
      transform: [
        { translateX: b.x - w / 2 + dx },
        { translateY: b.y - h / 2 - b.rise * up },
        { scale: s * b.scale },
        { rotate: `${b.tilt * Math.sin(p * 5 + b.phase)}deg` },
      ],
    };
  });
  const strip = useAnimatedStyle(() => ({ transform: [{ translateY: -slot.b.value.idx * h }] }));
  return (
    <Animated.View style={[{ position: 'absolute', left: 0, top: 0, width: w, height: h, overflow: 'hidden', opacity: 0 }, outer]}>
      <Animated.View style={strip}>
        {RESULT_REACTIONS.map((emoji) => (
          <Text key={emoji} allowFontScaling={false} style={{ width: w, height: h, lineHeight: h, fontSize: size, textAlign: 'center', includeFontPadding: false }}>
            {emoji}
          </Text>
        ))}
      </Animated.View>
    </Animated.View>
  );
});

// The bar.

const GAP = 6;
/** Where an emoji sits in its button, from the top of the bar. */
const EMOJI_Y = 22;
/** Bursts of other people's taps in one event, at most (the rest only count); my own taps always get one. */
const LIVE_BURSTS = 16;
/** Under reduced motion a count changes with a fade (not a pop). */
const COUNT_FADE = FadeIn.duration(220).reduceMotion(ReduceMotion.Never);

/** A count that fits under a button: 1 234 → 1,2k. */
const shortCount = (n: number) => (n < 1000 ? formatNumber(n) : `${formatNumber(Math.floor(n / 100) / 10, { maximumFractionDigits: 1 })}k`);
/** Under this window width the emojis shrink so they stay inside their buttons. */
const NARROW = 380;

const haptic = () => {
  if (Platform.OS !== 'web') Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
};

/**
 * The 8 reactions under a result: a tap sends one (a burst from that button), the counts under them, mine tinted.
 * Long-press one with taps: who reacted. Nothing on a server without `result_reactions`. `me`: my seat in the
 * result's room (the session's by default).
 */
export function LiveReactions({ roundId, byId, me }: { roundId: number; byId: Map<number, Member>; me?: number | null }) {
  const { resultReactions } = useFeatures();
  if (!resultReactions) return null;
  return <Bar roundId={roundId} byId={byId} me={me} />;
}

function Bar({ roundId, byId, me }: { roundId: number; byId: Map<number, Member>; me?: number | null }) {
  const tr = useT();
  const session = useSession();
  const seat = me !== undefined ? me : (session.me?.id ?? null);
  const { ids: blocked } = useBlocks();
  const { data, tap } = useResultReactions(roundId, seat);
  const { slots, fire } = useBurstPool(24);
  const width = useRef(0);
  const [who, setWho] = useState<string | null>(null);
  const narrow = useWindowDimensions().width < NARROW;

  /** The middle of `emoji`'s button, across the bar. */
  const centerOf = useCallback((emoji: string) => {
    const n = RESULT_REACTIONS.length;
    const cell = (width.current - GAP * (n - 1)) / n;
    return emojiIndex(emoji) * (cell + GAP) + cell / 2;
  }, []);
  const onTap = useCallback(
    (emoji: string) => {
      if (!tap(emoji)) return;
      haptic();
      fire(emoji, centerOf(emoji), EMOJI_Y);
    },
    [tap, fire, centerOf],
  );
  const onLive = useCallback(
    (taps: LiveTap[]) =>
      burstList(taps, LIVE_BURSTS).forEach((emoji, k) => fire(emoji, centerOf(emoji) + (Math.random() * 2 - 1) * 8, EMOJI_Y, k * 90)),
    [fire, centerOf],
  );
  useReactedListener(roundId, seat, blocked, onLive);

  const rows = new Map((data?.reactions ?? []).map((r) => [r.emoji, r]));
  const capped = (data?.mine ?? 0) >= (data?.cap ?? RESULT_CAP);
  return (
    <View accessibilityRole="toolbar" accessibilityLabel={tr('reactions.bar')}>
      <View
        style={{ flexDirection: 'row', gap: GAP }}
        onLayout={({ nativeEvent }) => {
          width.current = nativeEvent.layout.width;
        }}>
        {RESULT_REACTIONS.map((emoji) => {
          const r = rows.get(emoji);
          return <ReactionButton key={emoji} emoji={emoji} count={r?.count ?? 0} mine={r?.mine ?? 0} capped={capped} narrow={narrow} onTap={onTap} onWho={setWho} />;
        })}
      </View>
      <BurstLayer slots={slots} size={30} />
      <WhoTapped row={who ? (rows.get(who) ?? null) : null} byId={byId} onClose={() => setWho(null)} />
    </View>
  );
}

const ReactionButton = memo(function ReactionButton({
  emoji,
  count,
  mine,
  capped,
  narrow,
  onTap,
  onWho,
}: {
  emoji: string;
  count: number;
  mine: number;
  capped: boolean;
  narrow: boolean;
  onTap: (emoji: string) => void;
  onWho: (emoji: string) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const pop = usePop(count, 1.3, count > 0);
  const label = [tr('reactions.button', { emoji, count }), mine > 0 && tr('reactions.mine', { count: mine }), capped && tr('reactions.capped')]
    .filter(Boolean)
    .join(', ');
  return (
    <PressableScale
      scale={0.86}
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityHint={count > 0 ? tr('reactions.who') : undefined}
      accessibilityState={{ disabled: capped, selected: mine > 0 }}
      onPress={() => onTap(emoji)}
      onLongPress={count > 0 ? () => onWho(emoji) : undefined}
      delayLongPress={400}
      style={[
        {
          flex: 1,
          minWidth: 0,
          height: 62,
          borderRadius: 18,
          alignItems: 'center',
          justifyContent: 'center',
          borderWidth: 1.5,
          borderColor: mine > 0 ? t.accInk : 'transparent',
          backgroundColor: mine > 0 ? t.accSoft : t.card,
          // At the cap the whole bar fades back (taps do nothing, long-press still shows who).
          opacity: capped ? 0.5 : 1,
        },
        t.shadow,
      ]}>
      <Text allowFontScaling={false} style={{ fontSize: narrow ? 22 : 26, lineHeight: 32 }}>
        {emoji}
      </Text>
      <Animated.Text
        key={reduced ? count : 'count'}
        entering={reduced ? COUNT_FADE : undefined}
        numberOfLines={1}
        style={[
          { height: 16, fontFamily: t.font.display, fontSize: 13, lineHeight: 16, fontVariant: ['tabular-nums'], color: mine > 0 ? t.accInk : t.mut },
          pop,
        ]}>
        {count > 0 ? shortCount(count) : ''}
      </Animated.Text>
    </PressableScale>
  );
});

/** Who tapped one emoji and how many times, oldest first. Someone no longer in the room's list: a deleted account. */
function WhoTapped({ row, byId, onClose }: { row: ResultReaction | null; byId: Map<number, Member>; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  // The last one stays drawn while the dialog fades out.
  const [shown, setShown] = useState<ResultReaction | null>(null);
  if (row && row !== shown) setShown(row);
  return (
    <Dialog visible={!!row} onClose={onClose}>
      <View accessible style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Text style={{ fontSize: 26 }}>{shown?.emoji}</Text>
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('reactions.count', { count: shown?.count ?? 0 })}</Text>
      </View>
      <ScrollView style={{ maxHeight: 360 }} contentContainerStyle={{ gap: 4 }}>
        {(shown?.members ?? []).map(({ member_id, count }) => {
          const m = byId.get(member_id);
          return (
            <View key={member_id} accessible style={{ flexDirection: 'row', alignItems: 'center', gap: 12, height: 44 }}>
              <Avatar member={m} size={32} />
              <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: m ? t.txt : t.mut }}>
                {m?.name ?? tr('common.deletedAccount')}
              </Text>
              <Text style={{ fontFamily: t.font.display, fontSize: 15, color: t.mut, fontVariant: ['tabular-nums'] }}>×{formatNumber(count)}</Text>
            </View>
          );
        })}
      </ScrollView>
    </Dialog>
  );
}

// The big screen (a TV): no account, so no taps and no `GET`; the counts come with the party's state.

const NO_ONE: ReadonlySet<number> = new Set();
const SCREEN_BURSTS = 40;
const SCREEN: BurstStyle = { rise: [300, 500], drift: 60, wobble: 26, duration: [1400, 1900], scale: [0.8, 1.2] };

/**
 * Big bursts rising from the bottom edge of the screen for each tap on `roundId`, over everything. `wide`: the join
 * QR code is on the left, so they rise on the result's side only.
 */
export function ScreenBursts({ roundId, wide }: { roundId: number; wide: boolean }) {
  const { width, height } = useWindowDimensions();
  const { slots, fire } = useBurstPool(SCREEN_BURSTS, SCREEN);
  const [from, to] = wide ? [0.4, 0.96] : [0.06, 0.94];
  const onLive = useCallback(
    (taps: LiveTap[]) =>
      burstList(taps, SCREEN_BURSTS).forEach((emoji, k) =>
        fire(emoji, width * (from + Math.random() * (to - from)), height + 40, k * 70 + Math.random() * 60, [height * 0.3, height * 0.5]),
      ),
    [fire, width, height, from, to],
  );
  useReactedListener(roundId, null, NO_ONE, onLive);
  return <BurstLayer slots={slots} size={80} clip />;
}

/** The big screen's counts: one large chip per emoji tapped, palette order. */
export function ScreenCounts({ reactions, unit }: { reactions: { emoji: string; count: number }[] | null; unit: number }) {
  const counts = new Map((reactions ?? []).map((r) => [r.emoji, r.count]));
  const shown = RESULT_REACTIONS.filter((e) => (counts.get(e) ?? 0) > 0);
  if (!shown.length) return null;
  return (
    <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: Math.round(14 * unit) }}>
      {shown.map((emoji) => (
        <ScreenCount key={emoji} emoji={emoji} count={counts.get(emoji) ?? 0} unit={unit} />
      ))}
    </View>
  );
}

const ScreenCount = memo(function ScreenCount({ emoji, count, unit }: { emoji: string; count: number; unit: number }) {
  const t = useTheme();
  const tr = useT();
  const pop = usePop(count, 1.2);
  const big = (n: number) => Math.round(n * unit);
  return (
    <Animated.View
      accessible
      accessibilityLabel={tr('reactions.screen', { emoji, count })}
      style={[
        {
          height: big(88),
          paddingHorizontal: big(22),
          borderRadius: 999,
          flexDirection: 'row',
          alignItems: 'center',
          gap: big(12),
          backgroundColor: t.card,
          // A TV crushes dark greys: the edge keeps the chip visible on a dark background.
          borderWidth: 1,
          borderColor: t.line,
        },
        t.shadow,
        pop,
      ]}>
      <Text allowFontScaling={false} style={{ fontSize: big(48), lineHeight: big(58) }}>
        {emoji}
      </Text>
      <Text style={{ fontFamily: t.font.display, fontSize: big(40), color: t.txt, fontVariant: ['tabular-nums'] }}>{formatNumber(count)}</Text>
    </Animated.View>
  );
});
