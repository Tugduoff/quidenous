import * as Haptics from 'expo-haptics';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Platform, Pressable, StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import Animated, { FadeIn, useAnimatedStyle, useReducedMotion, useSharedValue, withTiming } from 'react-native-reanimated';

import { Dialog, DialogTitle } from '@/components/ui';
import { milestone as describe, PREVIEW_AT, playPreview, SPECIAL_NUMBERS, tier, usePreview } from '@/lib/milestones';
import { useTheme } from '@/lib/theme';

import { BigNumber, makeRand, useClock, useShake } from './engine';
import { numberSize, tidy } from './scene';
import { sceneFor } from './scenes';

/**
 * Full-screen party for a milestone, drawn over the app. Tap anywhere to skip; it also closes by itself.
 * Under reduced motion: a still card instead.
 */
export function Celebration({ count, onDone, frozen }: { count: number; onDone: () => void; frozen?: number | null }) {
  const reduced = useReducedMotion();
  const t = useTheme();
  const { width: w, height: h } = useWindowDimensions();
  const m = useMemo(() => describe(count), [count]);
  const scene = useMemo(() => sceneFor(m, t), [m, t]);
  const rand = useMemo(() => makeRand(count * 7919 + 17), [count]);
  const clock = useClock(scene.duration, reduced ? scene.duration : frozen);
  const shake = useShake(clock, scene.impacts);
  const out = useSharedValue(1);
  const closing = useRef(false);
  const fade = useAnimatedStyle(() => ({ opacity: out.value }));

  const close = () => {
    if (closing.current) return;
    closing.current = true;
    out.value = withTiming(0, { duration: 220 });
    setTimeout(onDone, 230);
  };

  useEffect(() => {
    if (frozen != null) return;
    const timers = [setTimeout(close, reduced ? 4500 : scene.duration)];
    // A buzz on each impact (Android/iOS only).
    if (!reduced && Platform.OS !== 'web')
      for (const i of scene.impacts)
        timers.push(
          setTimeout(
            () =>
              Haptics.impactAsync(
                i.amp >= 8 ? Haptics.ImpactFeedbackStyle.Heavy : i.amp >= 4 ? Haptics.ImpactFeedbackStyle.Medium : Haptics.ImpactFeedbackStyle.Light,
              ).catch(() => {}),
            i.at,
          ),
        );
    return () => timers.forEach(clearTimeout);
    // Once per party.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const { Render } = scene;
  const cx = w / 2;
  const cy = Math.round(h * 0.42);
  return (
    <Animated.View entering={FadeIn.duration(160)} style={[StyleSheet.absoluteFill, { zIndex: 100 }]}>
      <Animated.View style={[StyleSheet.absoluteFill, fade]}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={`${count} questions ! Toucher pour fermer`}
          onPress={close}
          style={StyleSheet.absoluteFill}>
          <View style={[StyleSheet.absoluteFill, { backgroundColor: reduced ? t.bg : scene.bg, opacity: reduced ? 0.96 : (scene.dim ?? 0.96) }]} />
          {reduced ? (
            <View style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center', padding: 32, gap: 8 }]}>
              <BigNumber value={count} size={numberSize(count)} color={t.accInk} font={t.font.display} />
              <Text style={{ fontFamily: t.font.display, fontSize: 28, letterSpacing: -0.5, color: t.txt }}>questions !</Text>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 22, color: t.mut, textAlign: 'center', maxWidth: 290 }}>
                {tidy(m.line)}
              </Text>
            </View>
          ) : (
            <Animated.View style={[StyleSheet.absoluteFill, shake]} pointerEvents="none">
              <Render clock={clock} m={m} t={t} w={w} h={h} rand={rand} cx={cx} cy={cy} />
            </Animated.View>
          )}
        </Pressable>
      </Animated.View>
    </Animated.View>
  );
}

/** Plays the hidden preview (`?fete=777`, or the room name long-pressed five times), signed in or not. */
export function CelebrationPreview() {
  const n = usePreview();
  if (n == null) return null;
  return <Celebration key={n} count={n} frozen={PREVIEW_AT} onDone={() => playPreview(null)} />;
}

/** Every milestone to preview: each 50 up to 2000 and the special numbers. */
const ALL = [...new Set([...SPECIAL_NUMBERS, ...Array.from({ length: 40 }, (_, i) => (i + 1) * 50)])].sort((a, b) => a - b);

/** Hidden picker (the room name long-pressed five times): plays any party without touching what this device has seen. */
export function MilestonePicker({ visible, onClose }: { visible: boolean; onClose: () => void }) {
  const t = useTheme();
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title="Paliers" />
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 6 }}>
        {ALL.map((n) => {
          const k = tier(n);
          return (
            <Pressable
              key={n}
              accessibilityRole="button"
              accessibilityLabel={`Fête ${n}`}
              onPress={() => {
                onClose();
                playPreview(n);
              }}
              style={{
                paddingHorizontal: 10,
                height: 30,
                borderRadius: t.radius.pill,
                justifyContent: 'center',
                backgroundColor: k === 'epic' ? t.acc : k === 'medium' ? t.accSoft : t.card2,
              }}>
              <Text style={{ fontFamily: t.font.bold, fontSize: 13, color: k === 'epic' ? t.accTxt : k === 'medium' ? t.accInk : t.txt }}>{n}</Text>
            </Pressable>
          );
        })}
      </View>
    </Dialog>
  );
}

/** Five long presses within 20 s open the picker. */
export function useSecretPicker() {
  const [open, setOpen] = useState(false);
  const presses = useRef<number[]>([]);
  const onLongPress = () => {
    const now = Date.now();
    presses.current = [...presses.current.filter((p) => now - p < 20_000), now];
    if (presses.current.length >= 5) {
      presses.current = [];
      setOpen(true);
    }
  };
  return { onLongPress, picker: <MilestonePicker visible={open} onClose={() => setOpen(false)} /> };
}
