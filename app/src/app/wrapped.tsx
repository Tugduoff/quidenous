import { router, useLocalSearchParams } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { Check, Share2, X } from 'lucide-react-native';
import { type ReactNode, useEffect, useRef, useState, useEffectEvent } from 'react';
import { AccessibilityInfo, ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';
import { GestureDetector, GestureHandlerRootView, usePanGesture } from 'react-native-gesture-handler';
import Animated, { cancelAnimation, Easing, FadeIn, FadeInDown, ReduceMotion, type SharedValue, useAnimatedStyle, useReducedMotion, useSharedValue, withSpring, withTiming, ZoomIn } from 'react-native-reanimated';
import { SafeAreaView } from 'react-native-safe-area-context';
import Svg, { Circle, Defs, LinearGradient, Rect, Stop } from 'react-native-svg';
import { scheduleOnRN } from 'react-native-worklets';
import { Mascot } from '@/components/mascot/mascot';
import { QueryState } from '@/components/ui/query-state';
import { Caps, iconFor } from '@/components/stats/common';
import { dateFormat, useT } from '@/lib/i18n';
import { LAYOUT_EASE, SPRING } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { shareWrappedCard, useRoomWrappedOk, useWrapped, type Wrapped, type WrappedCard, type WrappedScope } from '@/lib/api/wrapped';
import { Avatar } from '@/components/ui/avatar';
import type { Member } from '@/lib/api/group';
import { useFeatures } from '@/lib/api/features';

/** How long a card stays before the next one (auto-advance). */
const CARD_MS = 6000;
/** Text on the bright card colours (members' colours and the accents are all light). */
const INK = '#121216';
/** Cards with nobody on them: bold pairs of the app's accents, in turn. */
const PAIRS: [string, string][] = [
  ['#C8FF3D', '#5EF2B0'],
  ['#FF6DB5', '#FFB547'],
  ['#5AC8FA', '#B48CFF'],
  ['#FFB547', '#C8FF3D'],
  ['#B48CFF', '#FF6DB5'],
  ['#5EF2B0', '#5AC8FA'],
];
const sys = ReduceMotion.System;

const colorsOf = (card: WrappedCard, i: number): [string, string] => {
  const pair = PAIRS[i % PAIRS.length];
  const [p, q] = card.people;
  return p ? [p.color, q?.color ?? pair[1]] : pair;
};

/**
 * The big figure's size: a step by length, capped so a short figure stays on one line ("9 messages")
 * and a long one never breaks inside a word (~0.6 em per glyph of the display font, 320 pt wide).
 */
const bigSize = (s: string) => {
  const step = s.length <= 3 ? 116 : s.length <= 6 ? 92 : s.length <= 10 ? 64 : s.length <= 16 ? 48 : 38;
  const fit = s.length <= 12 ? s.length : Math.max(...s.split(' ').map((w) => w.length));
  return Math.min(step, Math.floor(320 / (fit * 0.6)));
};

/** Whether a screen reader is on: it reads the cards at its own pace (no auto-advance). */
function useScreenReader() {
  const [on, setOn] = useState(false);
  useEffect(() => {
    let live = true;
    AccessibilityInfo.isScreenReaderEnabled()
      .then((v) => live && setOn(v))
      .catch(() => {});
    const sub = AccessibilityInfo.addEventListener('screenReaderChanged', setOn);
    return () => {
      live = false;
      sub.remove();
    };
  }, []);
  return on;
}

/** The card's colours, full screen, with a few big shapes for rhythm (where they sit changes from card to card). */
function Backdrop({ card, index, year }: { card: WrappedCard; index: number; year: number }) {
  const t = useTheme();
  const [box, setBox] = useState({ w: 0, h: 0 });
  const [a, b] = colorsOf(card, index);
  const flip = index % 2 === 1;
  const id = `wg${index}`;
  return (
    <View
      pointerEvents="none"
      onLayout={(e) => setBox({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}
      style={[StyleSheet.absoluteFill, { backgroundColor: a, overflow: 'hidden' }]}>
      {box.w > 0 && (
        <Svg width={box.w} height={box.h} style={{ position: 'absolute' }}>
          <Defs>
            <LinearGradient id={id} x1={flip ? '1' : '0'} y1="0" x2={flip ? '0' : '1'} y2="1">
              <Stop offset="0" stopColor={a} />
              <Stop offset="1" stopColor={b} />
            </LinearGradient>
          </Defs>
          <Rect x="0" y="0" width={box.w} height={box.h} fill={`url(#${id})`} />
          <Circle cx={flip ? box.w * 0.1 : box.w * 0.92} cy={box.h * 0.16} r={box.w * 0.55} fill={INK} fillOpacity={0.06} />
          <Circle
            cx={flip ? box.w * 0.95 : box.w * 0.05}
            cy={box.h * 0.82}
            r={box.w * 0.32}
            fill="none"
            stroke={INK}
            strokeOpacity={0.08}
            strokeWidth={28}
          />
        </Svg>
      )}
      {/* The year, huge and faint, in a corner. */}
      <Text
        numberOfLines={1}
        style={{
          position: 'absolute',
          bottom: -36,
          [flip ? 'left' : 'right']: -12,
          fontFamily: t.font.display,
          fontSize: 180,
          letterSpacing: -8,
          color: INK,
          opacity: 0.07,
        }}>
        {year}
      </Text>
    </View>
  );
}

/** One thin bar per card: the past ones full, the current one filling (on the UI thread), the next ones empty. */
function Segment({ i, index, progress }: { i: number; index: number; progress: SharedValue<number> }) {
  const fill = useAnimatedStyle(() => ({ width: `${(i < index ? 1 : i > index ? 0 : progress.value) * 100}%` }));
  return (
    <View style={{ flex: 1, height: 3, borderRadius: 2, backgroundColor: '#1212162E', overflow: 'hidden' }}>
      <Animated.View style={[{ height: 3, borderRadius: 2, backgroundColor: INK }, fill]} />
    </View>
  );
}

/** Someone on a card, as an avatar can show them (a photo, a drawn avatar or their initial on their colour). */
const asMember = (p: WrappedCard['people'][number], i: number): Member => ({
  id: -1 - i,
  name: p.name,
  color: p.color,
  photo_url: p.photo_url,
  avatar: p.avatar,
});

/** What a card says: its title, the big figure, the punchline, the people and the card's mascot. */
function CardBody({ card, n, total }: { card: WrappedCard; n: number; total: number }) {
  const t = useTheme();
  const tr = useT();
  const Icon = iconFor(card.icon);
  const big = bigSize(card.big);
  const label = [tr('wrapped.page', { n, total }), card.title, card.question, card.big, card.line].filter(Boolean).join('. ');
  return (
    <View accessible accessibilityLabel={label} style={{ flex: 1, paddingHorizontal: 24, paddingTop: 12, paddingBottom: 8 }}>
      <Animated.View entering={FadeIn.duration(260).reduceMotion(sys)} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <Icon size={18} color={INK} strokeWidth={2.5} />
        <Caps color={INK} size={13} lines={2}>
          {card.title}
        </Caps>
      </Animated.View>
      <View style={{ flex: 1, alignItems: 'flex-end', justifyContent: 'center' }}>
        {card.mascot && (
          <Animated.View entering={ZoomIn.springify().damping(14).stiffness(200).delay(120).reduceMotion(sys)}>
            <Mascot who={card.mascot.who} mood={card.mascot.mood} size={168} />
          </Animated.View>
        )}
      </View>
      {card.people.length > 0 && (
        <Animated.View entering={FadeInDown.duration(320).delay(80).easing(LAYOUT_EASE).reduceMotion(sys)} style={{ flexDirection: 'row', marginBottom: 10 }}>
          {card.people.slice(0, 2).map((p, i) => (
            <View key={i} style={{ marginLeft: i ? -16 : 0, borderRadius: 40, borderWidth: 3, borderColor: INK }}>
              <Avatar member={asMember(p, i)} size={64} />
            </View>
          ))}
        </Animated.View>
      )}
      {!!card.question && (
        <Animated.View
          entering={FadeInDown.duration(320).delay(60).easing(LAYOUT_EASE).reduceMotion(sys)}
          style={{ marginBottom: 12, padding: 14, borderRadius: 18, backgroundColor: '#FFFFFF8C' }}>
          <Text numberOfLines={4} style={{ fontFamily: t.font.semibold, fontSize: 17, lineHeight: 23, color: INK }}>
            {card.question}
          </Text>
        </Animated.View>
      )}
      <Animated.View entering={FadeInDown.duration(380).delay(100).easing(LAYOUT_EASE).reduceMotion(sys)}>
        <Text
          numberOfLines={2}
          adjustsFontSizeToFit
          minimumFontScale={0.5}
          style={{ fontFamily: t.font.display, fontSize: big, lineHeight: big * 1.04, letterSpacing: big > 60 ? -3 : -1.5, color: INK }}>
          {card.big}
        </Text>
      </Animated.View>
      <Animated.View entering={FadeIn.duration(320).delay(240).reduceMotion(sys)}>
        <Text numberOfLines={4} style={{ marginTop: 6, fontFamily: t.font.semibold, fontSize: 20, lineHeight: 27, color: INK }}>
          {card.line}
        </Text>
      </Animated.View>
    </View>
  );
}

/** The share button of a shareable card (the server draws its picture). */
function ShareButton({ w, card }: { w: Wrapped; card: WrappedCard }) {
  const t = useTheme();
  const tr = useT();
  const [state, setState] = useState<'idle' | 'busy' | 'copied'>('idle');
  const share = async () => {
    setState('busy');
    const outcome = await shareWrappedCard(w, card);
    setState(outcome === 'copied' ? 'copied' : 'idle');
    if (outcome === 'copied') setTimeout(() => setState('idle'), 1800);
  };
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={state === 'copied' ? tr('wrapped.linkCopied') : tr('wrapped.share')}
      accessibilityState={{ busy: state === 'busy' }}
      disabled={state === 'busy'}
      onPress={share}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 8,
        height: 48,
        paddingHorizontal: 22,
        borderRadius: 999,
        backgroundColor: INK,
        opacity: pressed ? 0.8 : 1,
      })}>
      {state === 'busy' ? (
        <ActivityIndicator size="small" color="#FFFFFF" />
      ) : state === 'copied' ? (
        <Check size={18} color="#FFFFFF" />
      ) : (
        <Share2 size={18} color="#FFFFFF" />
      )}
      <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: '#FFFFFF' }}>{tr(state === 'copied' ? 'wrapped.linkCopied' : 'wrapped.share')}</Text>
    </Pressable>
  );
}

/**
 * The cards, one at a time: tap right / left (or swipe) to move, hold to pause. Each card moves on by itself after
 * CARD_MS, unless motion is reduced or a screen reader is on. The timer and the bars run on the UI thread.
 */
function Stories({ w, chrome }: { w: Wrapped; chrome: (ink: string) => ReactNode }) {
  const tr = useT();
  const reduced = useReducedMotion();
  const reader = useScreenReader();
  const auto = !reduced && !reader;
  const count = w.cards.length;
  const [index, setIndex] = useState(0);
  const [prev, setPrev] = useState<number | null>(null);
  const progress = useSharedValue(0);
  const drag = useSharedValue(0);
  // A swipe also ends as a press on the tap zones (the web): that press is ignored.
  const swiped = useRef(false);

  const go = (d: number) => {
    setIndex((i) => {
      const next = Math.max(0, Math.min(count - 1, i + d));
      if (next !== i) setPrev(i);
      return next;
    });
  };
  const advance = () => go(1);
  const close = () => (router.canGoBack() ? router.back() : router.replace('/'));

  // Fill the current bar from where it is; at the end, the next card (the last one just stays).
  const run = (from: number) => {
    progress.value = withTiming(1, { duration: CARD_MS * (1 - from), easing: Easing.linear, reduceMotion: ReduceMotion.Never }, (finished) => {
      'worklet';
      if (finished) scheduleOnRN(advance);
    });
  };
  const runFromStart = useEffectEvent(() => run(0));
  useEffect(() => {
    cancelAnimation(progress);
    if (!auto) {
      progress.value = 1;
      return;
    }
    progress.value = 0;
    if (index < count - 1) runFromStart();
    else progress.value = withTiming(1, { duration: CARD_MS, easing: Easing.linear, reduceMotion: ReduceMotion.Never });
  }, [index, auto, count, progress]);
  useEffect(() => {
    const card = w.cards[index];
    if (reader && card) AccessibilityInfo.announceForAccessibility(`${card.title}. ${card.big}. ${card.line}`);
  }, [index, reader, w.cards]);

  const pause = () => {
    swiped.current = false;
    if (auto) cancelAnimation(progress);
  };
  const resume = () => {
    if (!auto) return;
    const at = progress.value;
    if (at >= 1) return;
    if (index < count - 1) run(at);
    else progress.value = withTiming(1, { duration: CARD_MS * (1 - at), easing: Easing.linear, reduceMotion: ReduceMotion.Never });
  };
  const tap = (d: number) => {
    if (swiped.current) {
      swiped.current = false;
      return;
    }
    go(d);
  };
  const markSwiped = () => {
    swiped.current = true;
  };

  const pan = usePanGesture({
    activeOffsetX: [-14, 14],
    activeOffsetY: [-20, 20],
    onActivate: () => {
      'worklet';
      scheduleOnRN(markSwiped);
    },
    onUpdate: (e) => {
      'worklet';
      drag.value = e.translationX * 0.35;
    },
    onDeactivate: (e) => {
      'worklet';
      drag.value = withSpring(0, SPRING);
      if (e.translationY > 120 && Math.abs(e.translationY) > Math.abs(e.translationX)) scheduleOnRN(close);
      else if (e.translationX < -50 || e.velocityX < -600) scheduleOnRN(go, 1);
      else if (e.translationX > 50 || e.velocityX > 600) scheduleOnRN(go, -1);
    },
  });
  const dragged = useAnimatedStyle(() => ({ transform: [{ translateX: drag.value }] }));

  const card = w.cards[index];
  return (
    <View style={{ flex: 1 }}>
      {prev != null && w.cards[prev] && <Backdrop card={w.cards[prev]} index={prev} year={w.year} />}
      <Animated.View key={card.key} entering={FadeIn.duration(280).reduceMotion(sys)} style={StyleSheet.absoluteFill}>
        <Backdrop card={card} index={index} year={w.year} />
      </Animated.View>
      <StatusBar style="dark" />
      <SafeAreaView edges={['top', 'bottom']} style={{ flex: 1 }}>
        <View
          importantForAccessibility="no-hide-descendants"
          accessibilityElementsHidden
          style={{ flexDirection: 'row', gap: 4, paddingHorizontal: 12, paddingTop: 8 }}>
          {w.cards.map((c, i) => (
            <Segment key={c.key} i={i} index={index} progress={progress} />
          ))}
        </View>
        {chrome(INK)}
        <GestureDetector gesture={pan} touchAction="pan-y">
          <Animated.View style={[{ flex: 1 }, dragged]}>
            <CardBody key={card.key} card={card} n={index + 1} total={count} />
            {/* Tap zones: a third on the left goes back, the rest goes on; holding either pauses. */}
            <View style={[StyleSheet.absoluteFill, { flexDirection: 'row' }]}>
              <Pressable
                accessibilityRole="button"
                accessibilityLabel={tr('wrapped.prev')}
                accessibilityState={{ disabled: index === 0 }}
                onPressIn={pause}
                onPressOut={resume}
                onLongPress={() => {}}
                delayLongPress={250}
                onPress={() => tap(-1)}
                style={{ flex: 1 }}
              />
              <Pressable
                accessibilityRole="button"
                accessibilityLabel={tr('wrapped.next')}
                accessibilityState={{ disabled: index === count - 1 }}
                onPressIn={pause}
                onPressOut={resume}
                onLongPress={() => {}}
                delayLongPress={250}
                onPress={() => tap(1)}
                style={{ flex: 2 }}
              />
            </View>
          </Animated.View>
        </GestureDetector>
        <View style={{ height: 64, alignItems: 'center', justifyContent: 'center', paddingHorizontal: 24 }}>
          {card.shareable && <ShareButton key={card.key} w={w} card={card} />}
        </View>
      </SafeAreaView>
    </View>
  );
}

/** Two pills: my year or the room's. */
function ScopeToggle({ value, onChange, ink, track }: { value: WrappedScope; onChange: (s: WrappedScope) => void; ink: string; track: string }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View accessibilityRole="radiogroup" accessibilityLabel={tr('wrapped.scope.label')} style={{ flexDirection: 'row', padding: 3, gap: 2, borderRadius: 999, backgroundColor: track }}>
      {(['me', 'room'] as const).map((s) => {
        const on = s === value;
        return (
          <Pressable
            key={s}
            accessibilityRole="radio"
            accessibilityState={{ selected: on }}
            accessibilityLabel={tr(`wrapped.scope.${s}`)}
            onPress={() => !on && onChange(s)}
            hitSlop={{ top: 7, bottom: 7 }}
            style={{ height: 30, paddingHorizontal: 14, borderRadius: 999, alignItems: 'center', justifyContent: 'center', backgroundColor: on ? ink : 'transparent' }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: on ? (ink === INK ? '#FFFFFF' : t.bg) : ink }}>{tr(`wrapped.scope.${s}`)}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

/** Not a card: a mascot and a few words (out of season, not enough played, an older server). */
function Message({ who, mood, title, line }: { who: 'roupi' | 'pave'; mood: 'sleepy' | 'think'; title: string; line?: string }) {
  const t = useTheme();
  return (
    <Animated.View entering={FadeIn.duration(240).reduceMotion(sys)} style={{ flex: 1, alignItems: 'center', justifyContent: 'center', gap: 8, padding: 32 }}>
      <Mascot who={who} mood={mood} size={140} />
      <Text accessibilityRole="header" style={{ textAlign: 'center', fontFamily: t.font.display, fontSize: 26, lineHeight: 30, letterSpacing: -0.5, color: t.txt }}>
        {title}
      </Text>
      {!!line && <Text style={{ textAlign: 'center', fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{line}</Text>}
    </Animated.View>
  );
}

/** "1 décembre" from "2026-12-01". */
function day(iso: string) {
  const [y, m, d] = iso.split('-').map(Number);
  if (!y || !m || !d) return iso;
  return dateFormat({ day: 'numeric', month: 'long' }).format(new Date(y, m - 1, d));
}

/** « Ton année qui de nous » (or the room's): full-screen story cards. `?preview=1`: an admin looking early; `?scope=room`. */
export default function WrappedScreen() {
  const t = useTheme();
  const tr = useT();
  const params = useLocalSearchParams<{ preview?: string; scope?: string }>();
  const preview = params.preview === '1';
  const features = useFeatures();
  const roomOk = useRoomWrappedOk();
  const [chosen, setScope] = useState<WrappedScope>(params.scope === 'room' ? 'room' : 'me');
  // No room (or a party): only mine.
  const scope = roomOk ? chosen : 'me';
  const query = useWrapped(scope, preview);
  const { data: w } = query;
  const close = () => (router.canGoBack() ? router.back() : router.replace('/'));

  const chrome = (ink: string) => (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 12, paddingTop: 10, minHeight: 50 }}>
      {roomOk && <ScopeToggle value={scope} onChange={setScope} ink={ink} track={ink === INK ? '#1212161F' : t.card2} />}
      <View style={{ flex: 1 }} />
      {w?.preview && (
        <View style={{ paddingHorizontal: 10, height: 24, borderRadius: 12, justifyContent: 'center', borderWidth: 1.5, borderColor: ink }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.5, textTransform: 'uppercase', color: ink }}>{tr('wrapped.preview')}</Text>
        </View>
      )}
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('wrapped.close')}
        onPress={close}
        hitSlop={8}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: ink === INK ? '#1212161F' : t.card }}>
        <X size={20} color={ink} />
      </Pressable>
    </View>
  );

  const showing = !!w && (w.available || w.preview) && w.cards.length > 0;
  if (showing)
    return (
      <GestureHandlerRootView style={{ flex: 1, backgroundColor: t.bg }}>
        <Stories key={`${w.scope}-${w.year}`} w={w} chrome={chrome} />
      </GestureHandlerRootView>
    );

  let body: ReactNode;
  if (features.loaded && !features.wrapped) body = <Message who="roupi" mood="sleepy" title={tr('wrapped.unavailable')} line={tr('wrapped.noServer')} />;
  else if (!w) body = <QueryState query={query} style={{ flex: 1, justifyContent: 'center' }} />;
  else if (!w.available && !w.preview)
    body = <Message who="roupi" mood="sleepy" title={tr('wrapped.unavailable')} line={tr('wrapped.unavailableLine', { date: day(w.from) })} />;
  else
    body = <Message who="pave" mood="think" title={tr(scope === 'room' ? 'wrapped.emptyRoom' : 'wrapped.empty')} line={tr('wrapped.emptyLine')} />;
  return (
    <SafeAreaView edges={['top', 'bottom']} style={{ flex: 1, backgroundColor: t.bg }}>
      {chrome(t.txt)}
      {body}
    </SafeAreaView>
  );
}
