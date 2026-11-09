import { useEffect, useMemo, useRef } from 'react';
import { Animated, Easing, Pressable, StyleSheet, Text, useWindowDimensions, View } from 'react-native';

import { milestoneLine } from '@/lib/milestones';
import { useTheme } from '@/lib/theme';

const COLORS = ['#C8FF3D', '#FF4FA3', '#5AC8FA', '#FFB547', '#B48CFF', '#5EF2B0', '#FF7A45'];
const PIECES = 70;
const DURATION = 4200;

/** Full-screen confetti and a big number. Tap anywhere (or wait) to close. */
export function Celebration({ count, onDone }: { count: number; onDone: () => void }) {
  const t = useTheme();
  const { width, height } = useWindowDimensions();
  const fall = useRef(new Animated.Value(0)).current;
  const pop = useRef(new Animated.Value(0)).current;
  const fade = useRef(new Animated.Value(0)).current;

  const pieces = useMemo(
    () =>
      Array.from({ length: PIECES }, (_, i) => ({
        x: Math.random() * width,
        drift: (Math.random() - 0.5) * 160,
        delay: Math.random() * 0.35,
        spin: (Math.random() < 0.5 ? -1 : 1) * (360 + Math.random() * 720),
        w: 6 + Math.random() * 6,
        h: 10 + Math.random() * 10,
        round: i % 4 === 0,
        color: COLORS[i % COLORS.length],
      })),
    [width],
  );

  useEffect(() => {
    Animated.parallel([
      Animated.timing(fade, { toValue: 1, duration: 200, useNativeDriver: true }),
      Animated.timing(fall, { toValue: 1, duration: DURATION, easing: Easing.out(Easing.quad), useNativeDriver: true }),
      Animated.sequence([Animated.delay(150), Animated.spring(pop, { toValue: 1, friction: 4, tension: 60, useNativeDriver: true })]),
    ]).start();
    const timer = setTimeout(close, DURATION + 1200);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function close() {
    Animated.timing(fade, { toValue: 0, duration: 250, useNativeDriver: true }).start(() => onDone());
  }

  return (
    <Animated.View style={[StyleSheet.absoluteFill, { opacity: fade, zIndex: 100 }]}>
      <Pressable accessibilityRole="button" accessibilityLabel="Fermer" onPress={close} style={StyleSheet.absoluteFill}>
        <View style={[StyleSheet.absoluteFill, { backgroundColor: t.bg, opacity: 0.88 }]} />
        {pieces.map((p, i) => {
          // Each piece starts late by `delay`, then falls past the bottom.
          const progress = fall.interpolate({ inputRange: [0, p.delay, 1], outputRange: [0, 0, 1], extrapolate: 'clamp' });
          return (
            <Animated.View
              key={i}
              style={{
                position: 'absolute',
                left: p.x,
                top: -30,
                width: p.w,
                height: p.round ? p.w : p.h,
                borderRadius: p.round ? p.w / 2 : 2,
                backgroundColor: p.color,
                transform: [
                  { translateY: progress.interpolate({ inputRange: [0, 1], outputRange: [0, height + 60] }) },
                  { translateX: progress.interpolate({ inputRange: [0, 1], outputRange: [0, p.drift] }) },
                  { rotate: progress.interpolate({ inputRange: [0, 1], outputRange: ['0deg', `${p.spin}deg`] }) },
                ],
              }}
            />
          );
        })}
        <View style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center', padding: 32 }]} pointerEvents="none">
          <Animated.View
            style={{
              alignItems: 'center',
              gap: 10,
              transform: [{ scale: pop.interpolate({ inputRange: [0, 1], outputRange: [0.3, 1] }) }],
              opacity: pop.interpolate({ inputRange: [0, 0.3, 1], outputRange: [0, 1, 1] }),
            }}>
            <Text style={{ fontFamily: t.font.display, fontSize: 120, lineHeight: 124, letterSpacing: -4, color: t.accInk }}>{count}</Text>
            <Text style={{ fontFamily: t.font.display, fontSize: 28, letterSpacing: -0.5, color: t.txt }}>questions !</Text>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 22, color: t.mut, textAlign: 'center', maxWidth: 280 }}>
              {milestoneLine(count)}
            </Text>
          </Animated.View>
        </View>
      </Pressable>
    </Animated.View>
  );
}
