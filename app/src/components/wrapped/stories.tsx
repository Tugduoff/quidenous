import { StatusBar } from 'expo-status-bar';
import { type ReactNode, useRef, useState } from 'react';
import { Pressable, StyleSheet, View } from 'react-native';
import { GestureDetector, usePanGesture } from 'react-native-gesture-handler';
import Animated, { FadeIn, useAnimatedStyle, useReducedMotion, useSharedValue, withSpring } from 'react-native-reanimated';
import { SafeAreaView } from 'react-native-safe-area-context';
import { scheduleOnRN } from 'react-native-worklets';
import { useT } from '@/lib/i18n';
import { SPRING } from '@/lib/motion';
import type { Wrapped } from '@/lib/api/wrapped';
import { useAnnounce, useScreenReaderEnabled } from '@/lib/hooks/use-screen-reader';
import { useStoryProgress } from '@/lib/hooks/use-story-progress';
import { Backdrop } from '@/components/wrapped/backdrop';
import { CardBody } from '@/components/wrapped/card-body';
import { ShareButton } from '@/components/wrapped/share-button';
import { StorySegment } from '@/components/wrapped/story-segment';
import { closeWrapped, INK, sys } from '@/components/wrapped/constants';
import { clamp } from '@/lib/utils/easing';

const CARD_MS = 6000;

export function Stories({ w, chrome }: { w: Wrapped; chrome: (ink: string) => ReactNode }) {
  const tr = useT();
  const reduced = useReducedMotion();
  const reader = useScreenReaderEnabled();
  const auto = !reduced && !reader;
  const count = w.cards.length;
  const [index, setIndex] = useState(0);
  const [prev, setPrev] = useState<number | null>(null);
  const drag = useSharedValue(0);
  const swiped = useRef(false);

  const go = (d: number) => {
    setIndex((i) => {
      const next = clamp(i + d, 0, count - 1);
      if (next !== i) setPrev(i);
      return next;
    });
  };
  const advance = () => go(1);

  const story = useStoryProgress({ index, count, auto, cardMs: CARD_MS, onEnd: advance });
  const { progress, resume } = story;
  const shown = w.cards[index];
  useAnnounce(reader && shown ? `${shown.title}. ${shown.big}. ${shown.line}` : null);

  const pause = () => {
    swiped.current = false;
    story.pause();
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
      if (e.translationY > 120 && Math.abs(e.translationY) > Math.abs(e.translationX)) scheduleOnRN(closeWrapped);
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
            <StorySegment key={c.key} i={i} index={index} progress={progress} />
          ))}
        </View>
        {chrome(INK)}
        <GestureDetector gesture={pan} touchAction="pan-y">
          <Animated.View style={[{ flex: 1 }, dragged]}>
            <CardBody key={card.key} card={card} n={index + 1} total={count} />
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
