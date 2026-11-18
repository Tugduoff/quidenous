import { ThumbsDown, ThumbsUp } from 'lucide-react-native';
import { GestureDetector, usePanGesture } from 'react-native-gesture-handler';
import Animated, { interpolate, useAnimatedStyle, useReducedMotion, useSharedValue, withSpring, withTiming } from 'react-native-reanimated';
import { scheduleOnRN } from 'react-native-worklets';
import type { Suggestion, SuggestionVote } from '@/lib/api/community';
import { EASE, motion, SPRING } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { QuestionText } from '@/components/ui/text';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';
import { useWhen } from '@/lib/hooks/use-on-change';

/** Height of the card stack; the card behind peeks out under the top one. */
export const STACK = 210;
export const PEEK = 10;
/** How far a card must be dragged (or flung) to count as a vote. */
const SWIPE = 110;

/** One suggestion card. Only the top one can be dragged; the one behind sits a little smaller and lower. */
export function SwipeCard({ s, top, fly, onDecide }: { s: Suggestion; top: boolean; fly: SuggestionVote | null; onDecide: (v: SuggestionVote) => void }) {
  const t = useTheme();
  const reduced = useReducedMotion();
  const tx = useSharedValue(0);
  const ty = useSharedValue(0);
  const depth = useSharedValue(top ? 0 : 1);
  useAnimateOnChange(depth, top, () => withSpring(top ? 0 : 1, SPRING));

  // Off the screen to the right (up), left (down) or upwards (skipped), then the vote.
  const away = (v: SuggestionVote) => {
    if (reduced) return onDecide(v);
    const done = (finished?: boolean) => {
      'worklet';
      if (finished) scheduleOnRN(onDecide, v);
    };
    const cfg = { duration: 220, easing: EASE };
    if (v === 0) ty.set(withTiming(-STACK * 1.6, cfg, done));
    else tx.set(withTiming(v * 520, cfg, done));
  };
  useWhen(fly != null, () => fly != null && away(fly));

  const pan = usePanGesture({
    enabled: top,
    // Sideways only: dragging up and down still scrolls the page.
    activeOffsetX: [-12, 12],
    failOffsetY: [-14, 14],
    onUpdate: (e) => {
      'worklet';
      tx.set(e.translationX);
    },
    onDeactivate: (e) => {
      'worklet';
      const x = e.translationX;
      const flung = Math.abs(x) > 40 && Math.abs(e.velocityX) > 800 && Math.sign(x) === Math.sign(e.velocityX);
      if (Math.abs(x) > SWIPE || flung) {
        const v: SuggestionVote = x > 0 ? 1 : -1;
        tx.set(
          withTiming(v * 520, { duration: 180, easing: EASE }, (finished) => {
            'worklet';
            if (finished) scheduleOnRN(onDecide, v);
          }),
        );
      } else tx.set(withSpring(0, SPRING));
    },
  });

  const card = useAnimatedStyle(() => ({
    transform: [
      { translateX: tx.value },
      { translateY: ty.value + depth.value * PEEK },
      { rotate: `${tx.value / 22}deg` },
      { scale: 1 - depth.value * 0.05 },
    ],
    opacity: 1 - depth.value * 0.4,
  }));
  const upHint = useAnimatedStyle(() => ({ opacity: interpolate(tx.value, [20, SWIPE], [0, 1], 'clamp') }));
  const downHint = useAnimatedStyle(() => ({ opacity: interpolate(tx.value, [-SWIPE, -20], [1, 0], 'clamp') }));
  const long = s.text.length > 90;
  const hint = { position: 'absolute', top: 14, width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center' } as const;

  return (
    <GestureDetector gesture={pan}>
      <Animated.View
        entering={top ? undefined : motion.enter}
        style={[
          {
            position: 'absolute',
            left: 0,
            right: 0,
            top: 0,
            height: STACK,
            borderRadius: t.radius.card,
            backgroundColor: t.card,
            padding: 22,
            justifyContent: 'center',
          },
          t.shadow,
          card,
        ]}>
        <QuestionText
          text={s.text}
          rank={s.kind === 'rank'}
          style={{ fontFamily: t.font.display, fontSize: long ? 19 : 23, lineHeight: long ? 23 : 27, letterSpacing: -0.4, color: t.txt }}
        />
        <Animated.View style={[hint, { right: 14, backgroundColor: t.acc }, upHint]}>
          <ThumbsUp size={18} color={t.accTxt} />
        </Animated.View>
        <Animated.View style={[hint, { left: 14, backgroundColor: t.card2 }, downHint]}>
          <ThumbsDown size={18} color={t.txt} />
        </Animated.View>
      </Animated.View>
    </GestureDetector>
  );
}
