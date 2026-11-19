import { useMemo, useRef, useState } from 'react';
import { Pressable, StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import Animated, { FadeIn, useAnimatedStyle, useReducedMotion, useSharedValue, withTiming } from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { useLang, useT } from '@/lib/i18n';
import { milestone as describe, SPECIAL_NUMBERS, streakMilestone, tier } from '@/lib/utils/milestones';
import { tidy } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { makeRand } from '@/lib/utils/rand';
import { BigNumber } from '@/components/celebration/numbers';
import { useCelebrationClock, useCelebrationTimers } from '@/lib/hooks/use-celebration';
import { useShake } from '@/lib/hooks/use-shake';
import { numberSize } from '@/components/celebration/scene';
import { sceneFor } from '@/components/celebration/scenes';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { PREVIEW_AT, playPreview, STREAK_PREVIEW, usePreview } from '@/lib/hooks/use-milestone';

export function Celebration({ count, onDone, frozen, streak }: { count: number; onDone: () => void; frozen?: number | null; streak?: boolean }) {
  const reduced = useReducedMotion();
  const t = useTheme();
  const tr = useT();
  const { lang } = useLang();
  const { width: w, height: h } = useWindowDimensions();
  const m = useMemo(() => (streak ? streakMilestone(count, lang) : describe(count, lang)), [count, lang, streak]);
  const scene = useMemo(() => sceneFor(m, t), [m, t]);
  const rand = useMemo(() => makeRand(count * 7919 + 17), [count]);
  const clock = useCelebrationClock(scene.duration, reduced ? scene.duration : frozen);
  const shake = useShake(clock, scene.impacts);
  const out = useSharedValue(1);
  const closing = useRef(false);
  const fade = useAnimatedStyle(() => ({ opacity: out.value }));

  const close = () => {
    if (closing.current) return;
    closing.current = true;
    out.set(withTiming(0, { duration: 220 }));
    setTimeout(onDone, 230);
  };

  useCelebrationTimers({ frozen, reduced, duration: scene.duration, impacts: scene.impacts, onEnd: close });

  const { Render } = scene;
  const cx = w / 2;
  const cy = Math.round(h * 0.42);
  return (
    <Animated.View entering={FadeIn.duration(160)} style={[StyleSheet.absoluteFill, { zIndex: 100 }]}>
      <Animated.View style={[StyleSheet.absoluteFill, fade]}>
        <Pressable accessibilityRole="button" accessibilityLabel={tr(streak ? 'streaks.party.close' : 'celebration.close', { count })} onPress={close} style={StyleSheet.absoluteFill}>
          <View style={[StyleSheet.absoluteFill, { backgroundColor: reduced ? t.bg : scene.bg, opacity: reduced ? 0.96 : (scene.dim ?? 0.96) }]} />
          {reduced ? (
            <View style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center', padding: 32, gap: 8 }]}>
              {streak && <Mascot who="paillette" mood="celebrate" size={120} />}
              <BigNumber value={count} size={numberSize(count)} color={t.accInk} font={t.font.display} />
              <Text style={{ fontFamily: t.font.display, fontSize: 28, letterSpacing: -0.5, color: t.txt }}>{m.unit ?? tr('celebration.questions')}</Text>
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

export function CelebrationPreview() {
  const n = usePreview();
  const [streak, setStreak] = useState(STREAK_PREVIEW);
  if (streak != null) return <Celebration streak count={streak} frozen={PREVIEW_AT} onDone={() => setStreak(null)} />;
  if (n == null) return null;
  return <Celebration key={n} count={n} frozen={PREVIEW_AT} onDone={() => playPreview(null)} />;
}

const ALL = [...new Set([...SPECIAL_NUMBERS, ...Array.from({ length: 40 }, (_, i) => (i + 1) * 50)])].sort((a, b) => a - b);

function MilestonePicker({ visible, onClose }: { visible: boolean; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={tr('celebration.picker')} />
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 6 }}>
        {ALL.map((n) => {
          const k = tier(n);
          return (
            <Pressable
              key={n}
              accessibilityRole="button"
              accessibilityLabel={tr('celebration.pick', { n })}
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
