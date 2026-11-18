// <Mascot/>: one of the five friends of the band, drawn from lib/mascot.ts. Decorative (hidden from screen readers:
// the copy next to it carries the meaning). Static when small, when motion is reduced or when its screen is not focused.
//
// Animation runs on the UI thread from ONE shared clock (a looping shared value in seconds): every mascot derives its
// breathing, blinking, hops and prop motion from it with its own phase, so idle loops cost no JS and no re-render.
// The clock only ticks while at least one live mascot is mounted.
import { useIsFocused } from 'expo-router';
import { memo, useId } from 'react';
import { type StyleProp, View, type ViewStyle } from 'react-native';
import Animated, {
  useAnimatedProps,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withSequence,
  withSpring,
  withTiming,
} from 'react-native-reanimated';
import Svg, { Circle, ClipPath, Defs, Ellipse, G, Path, Rect } from 'react-native-svg';

import { compose, type Composed, type Group, GROUND, type Mood, type Pose, type Prop, type Shape, type Who } from '@/lib/utils/mascot';
import { useScheme } from '@/lib/theme';
import { mascotClock, useMascotClock } from '@/lib/hooks/use-mascot-clock';
import { useOnChange } from '@/lib/hooks/use-on-change';

/** Focused screen, or true outside the route tree (the root error boundary). */
function useFocusedSafe() {
  try {
    return useIsFocused();
  } catch {
    return true;
  }
}

// ---------- motion per mood (units of the 200 box) ----------
type Kind = 'idle' | 'happy' | 'accuse' | 'sleepy' | 'celebrate' | 'sad' | 'think' | 'caught';
const TAU = Math.PI * 2;
const BLINK_EVERY: Record<Who, number> = { zeste: 4, bulle: 5, pave: 6, paillette: 3, roupi: 5 };
const PHASE: Record<Who, number> = { zeste: 0, bulle: 1.7, pave: 0.9, paillette: 2.6, roupi: 3.4 };

/** Height of the celebrate double hop (0..1) at time t. */
function hop(t: number) {
  'worklet';
  const u = t % 3;
  if (u < 0.42) return Math.sin((Math.PI * u) / 0.42);
  if (u < 0.76) return 0.55 * Math.sin((Math.PI * (u - 0.42)) / 0.34);
  return 0;
}

const AG = Animated.createAnimatedComponent(G);

/** One part shape. Unlike the avatars, a shape with no fill is a line: fill none (SVG would fill it black). */
function shape(s: Shape, i: number) {
  const paint = {
    fill: s.fill ?? 'none',
    opacity: s.op,
    ...(s.stroke && { stroke: s.stroke, strokeWidth: s.sw, strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const }),
  };
  switch (s.el) {
    case 'path':
      return <Path key={i} d={s.d} {...paint} />;
    case 'circle':
      return <Circle key={i} cx={s.cx} cy={s.cy} r={s.r} {...paint} />;
    case 'ellipse':
      return <Ellipse key={i} cx={s.cx} cy={s.cy} rx={s.rx} ry={s.ry} {...paint} />;
    case 'rect':
      return <Rect key={i} x={s.x} y={s.y} width={s.w} height={s.h} rx={s.rx} {...paint} />;
  }
}

const Shapes = ({ g }: { g: Group }) => <G transform={g.tr}>{g.shapes.map(shape)}</G>;

export type MascotProps = {
  who: Who;
  /** A mood, or a full custom pose (the group shot). */
  mood?: Mood | Pose;
  /** Square size in px. Under 48 the drawing drops its finest details. */
  size?: number;
  /** Idle loop (blink, breathe, mood moves). Off by default under 40 px. */
  animate?: boolean;
  /** Overrides the mood's prop ('none' hides it). */
  prop?: Prop;
  /** Repaints the body (#RRGGBB), e.g. the user's accent. */
  tint?: string;
  /** How a custom pose moves (moods move on their own). */
  motion?: Kind;
  style?: StyleProp<ViewStyle>;
};

export const Mascot = memo(function Mascot({ who, mood = 'happy', size = 96, animate, prop, tint, motion, style }: MascotProps) {
  const lod = size < 48 ? 'lo' : 'hi';
  const c = compose(who, mood, { lod, prop, tint });
  const kind: Kind = motion ?? (typeof mood === 'string' ? mood : 'idle');
  const dark = useScheme() === 'dark';
  const reduced = useReducedMotion();
  const focused = useFocusedSafe();
  const live = (animate ?? size >= 40) && !reduced && focused;

  const on = useMascotClock(live);

  // Mood change: a quick squash-and-stretch while the parts swap (never on mount).
  const pop = useSharedValue(0);
  const moodKey = typeof mood === 'string' ? mood : JSON.stringify(mood);
  useOnChange(moodKey, () => {
    if (!reduced) pop.set(withSequence(withTiming(1, { duration: 80 }), withSpring(0, { damping: 9, stiffness: 320, mass: 0.6 })));
  });

  const k = size / 200;
  const phase = PHASE[who];
  const blinkEvery = BLINK_EVERY[who];
  const point = c.point;

  const bodyStyle = useAnimatedStyle(() => {
    const p = pop.value;
    if (!on.value) {
      return { transform: [{ translateX: 0 }, { translateY: 0 }, { rotate: '0deg' }, { scaleX: 1 + 0.1 * p }, { scaleY: 1 - 0.1 * p }] };
    }
    const t = mascotClock.value + phase;
    let tx = 0;
    let ty = 0;
    let rot = 0;
    let breath = 0.018 * Math.sin((TAU * t) / 2.4);
    if (kind === 'sleepy') {
      breath = 0.035 * Math.sin((TAU * t) / 4);
      rot = 1.6 * Math.sin((TAU * t) / 8);
    } else if (kind === 'sad') {
      breath = 0.02 * Math.sin((TAU * t) / 4);
      ty = 1.2 + 1.2 * Math.sin((TAU * t) / 4);
    } else if (kind === 'happy' || kind === 'idle') {
      rot = 1.2 * Math.sin((TAU * t) / 4);
    } else if (kind === 'celebrate') {
      const h = hop(t);
      ty = -16 * h;
      breath = 0.05 * h;
    } else if (kind === 'accuse') {
      const u = t % 2.4;
      const j = u < 0.32 ? Math.sin((Math.PI * u) / 0.32) : 0;
      tx = 4 * j * (point || 1);
      rot = 2.5 * j * (point || 1);
    } else if (kind === 'think') {
      ty = -1.2 * Math.max(0, Math.sin((TAU * t) / 0.8));
    } else if (kind === 'caught') {
      const u = t % 2;
      tx = u < 0.45 ? 1.6 * Math.sin(TAU * u * 11) : 0;
    }
    return {
      transform: [
        { translateX: tx * k },
        { translateY: ty * k },
        { rotate: `${rot}deg` },
        { scaleX: (1 - breath * 0.5) * (1 + 0.1 * p) },
        { scaleY: (1 + breath) * (1 - 0.1 * p) },
      ],
    };
  });

  // The shadow stays on the ground: smaller and lighter while the body is in the air. Dark mode: a faint light pool.
  const shadowOp = dark ? (c.shadow.air ? 0.05 : 0.07) : c.shadow.air ? 0.11 : 0.16;
  const shadowStyle = useAnimatedStyle(() => {
    const h = on.value && kind === 'celebrate' ? hop(mascotClock.value + phase) : 0;
    return { transform: [{ scaleX: 1 - 0.3 * h }, { scaleY: 1 - 0.3 * h }], opacity: shadowOp * (1 - 0.4 * h) };
  });

  const blinkT = (t: number) => {
    'worklet';
    const u = (t + phase * 0.7) % blinkEvery;
    // one blink per period, a double blink every third one
    const n = Math.floor((t + phase * 0.7) / blinkEvery);
    return u < 0.13 || (n % 3 === 0 && u > 0.26 && u < 0.38);
  };
  const eyesOpen = useAnimatedProps(() => ({ opacity: on.value && blinkT(mascotClock.value) ? 0 : 1 }));
  const eyesShut = useAnimatedProps(() => ({ opacity: on.value && blinkT(mascotClock.value) ? 1 : 0 }));

  const propKind = c.prop?.kind;
  const propStyle = useAnimatedStyle(() => {
    if (!on.value || !propKind) return { opacity: 1, transform: [{ translateX: 0 }, { translateY: 0 }, { scale: 1 }, { rotate: '0deg' }] };
    const t = mascotClock.value + phase;
    let tx = 0;
    let ty = 0;
    let s = 1;
    let rot = 0;
    let op = 1;
    if (propKind === 'zzz') {
      const u = (t % 3) / 3;
      ty = -10 * u;
      tx = 5 * u;
      op = u < 0.15 ? 0.3 + (0.7 * u) / 0.15 : u > 0.7 ? 1 - (u - 0.7) / 0.3 : 1;
    } else if (propKind === 'ask') {
      s = 1 + 0.08 * Math.max(0, Math.sin((TAU * t) / 1.5));
      rot = 6 * Math.sin((TAU * t) / 3);
    } else if (propKind === 'sparkles') {
      s = 1 + 0.06 * Math.sin((TAU * t) / 1.5);
      op = 0.75 + 0.25 * Math.sin((TAU * t) / 0.75);
    } else {
      // tears, sweat: a slow drip
      const u = (t % 2.4) / 2.4;
      ty = 7 * u;
      op = u > 0.75 ? 1 - (u - 0.75) / 0.25 : 1;
    }
    return { opacity: op, transform: [{ translateX: tx * k }, { translateY: ty * k }, { scale: s }, { rotate: `${rot}deg` }] };
  });

  const clip = `mq${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const sh = c.shadow;
  const box = { position: 'absolute', left: 0, top: 0, width: size, height: size } as const;
  const origin = `50% ${(GROUND / 200) * 100}%`;
  const propOrigin = c.prop ? `${c.prop.pivot[0] / 2}% ${c.prop.pivot[1] / 2}%` : '50% 50%';

  return (
    <View
      style={[{ width: size, height: size }, style]}
      pointerEvents="none"
      accessible={false}
      aria-hidden
      importantForAccessibility="no-hide-descendants"
      accessibilityElementsHidden>
      <Animated.View
        style={[
          {
            position: 'absolute',
            left: (sh.cx - sh.rx) * k,
            top: (GROUND + 1 - sh.ry) * k,
            width: sh.rx * 2 * k,
            height: sh.ry * 2 * k,
            borderRadius: sh.rx * k,
            backgroundColor: dark ? '#FFFFFF' : '#121216',
          },
          shadowStyle,
        ]}
      />
      <Animated.View style={[box, { transformOrigin: origin }, bodyStyle]}>
        <Body c={c} clip={clip} size={size} eyesOpen={eyesOpen} eyesShut={eyesShut} />
        {c.prop && c.prop.at !== 'abs' ? (
          <Animated.View style={[box, { transformOrigin: propOrigin }, propStyle]}>
            <Svg width={size} height={size} viewBox="0 0 200 200">
              <G transform={c.body}>
                <Shapes g={c.prop} />
              </G>
            </Svg>
          </Animated.View>
        ) : null}
      </Animated.View>
      {c.prop?.at === 'abs' ? (
        <Animated.View style={[box, propStyle]}>
          <Svg width={size} height={size} viewBox="0 0 200 200">
            <Shapes g={c.prop} />
          </Svg>
        </Animated.View>
      ) : null}
    </View>
  );
});

type AP = ReturnType<typeof useAnimatedProps<{ opacity: number }>>;

/** The static drawing: one Svg, ~25-40 elements. Re-renders only when the mood (or size class) changes. */
const Body = memo(function Body({ c, clip, size, eyesOpen, eyesShut }: { c: Composed; clip: string; size: number; eyesOpen: AP; eyesShut: AP }) {
  const f = c.fill;
  return (
    <Svg width={size} height={size} viewBox="0 0 200 200">
      <G transform={c.body}>
        {c.back.map((g, i) => (
          <Shapes key={i} g={g} />
        ))}
        {f.gloss ? (
          <>
            <Defs>
              <ClipPath id={clip}>
                <Path d={f.d} />
              </ClipPath>
            </Defs>
            <Path d={f.d} fill={f.shade} />
            <G clipPath={`url(#${clip})`}>
              <Path d={f.d} fill={f.color} transform="translate(-10 -9)" />
            </G>
            <Path d={f.gloss} fill="none" stroke="#FFFFFF" strokeWidth={6} strokeLinecap="round" opacity={0.75} />
            <Path d={f.d} fill="none" stroke={f.outline} strokeWidth={f.sw} strokeLinejoin="round" />
          </>
        ) : (
          <Path d={f.d} fill={f.color} stroke={f.outline} strokeWidth={f.sw} strokeLinejoin="round" />
        )}
        {c.over.map((g, i) => (
          <Shapes key={i} g={g} />
        ))}
        <Shapes g={c.under} />
        {c.blink ? (
          <>
            <AG animatedProps={eyesOpen}>
              <Shapes g={c.eyes} />
            </AG>
            <AG animatedProps={eyesShut} opacity={0}>
              <Shapes g={c.blink} />
            </AG>
          </>
        ) : (
          <Shapes g={c.eyes} />
        )}
        <Shapes g={c.face} />
        {c.front.map((g, i) => (
          <Shapes key={i} g={g} />
        ))}
      </G>
    </Svg>
  );
});

