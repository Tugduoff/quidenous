import { useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, LinearGradient, Path, RadialGradient, Rect, Stop } from 'react-native-svg';

import { clamp01 } from '@/lib/utils/easing';
import { raysPath } from '@/lib/utils/svg-paths';

import type { Clock } from '@/components/celebration/engine';

export function Shockwave({
  clock,
  at,
  x,
  y,
  color,
  size = 360,
  dur = 650,
  width = 6,
}: {
  clock: Clock;
  at: number;
  x: number;
  y: number;
  color: string;
  size?: number;
  dur?: number;
  width?: number;
}) {
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - at) / dur);
    const on = clock.value >= at && p < 1;
    const ease = 1 - Math.pow(1 - p, 3);
    return { opacity: on ? (1 - p) * 0.9 : 0, transform: [{ scale: 0.05 + ease * 0.95 }] };
  });
  return (
    <Animated.View
      pointerEvents="none"
      style={[
        {
          position: 'absolute',
          left: x - size / 2,
          top: y - size / 2,
          width: size,
          height: size,
          borderRadius: size / 2,
          borderWidth: width,
          borderColor: color,
        },
        style,
      ]}
    />
  );
}

export function Flash({
  clock,
  at,
  color,
  peak = 0.6,
  rise = 40,
  fall = 420,
}: {
  clock: Clock;
  at: number;
  color: string;
  peak?: number;
  rise?: number;
  fall?: number;
}) {
  const style = useAnimatedStyle(() => ({
    opacity: interpolate(clock.value, [at - rise, at, at + fall], [0, peak, 0], Extrapolation.CLAMP),
  }));
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: color }, style]} />;
}

export function Rays({
  clock,
  at,
  x,
  y,
  color,
  size = 620,
  count = 14,
  opacity = 0.22,
  speed = 18,
}: {
  clock: Clock;
  at: number;
  x: number;
  y: number;
  color: string;
  size?: number;
  count?: number;
  opacity?: number;
  speed?: number;
}) {
  const path = useMemo(() => raysPath(size, count), [size, count]);
  const style = useAnimatedStyle(() => {
    const ms = clock.value - at;
    const p = clamp01(ms / 500);
    return {
      opacity: p * opacity,
      transform: [{ rotate: `${(Math.max(0, ms) / 1000) * speed}deg` }, { scale: 0.6 + 0.4 * (1 - Math.pow(1 - p, 3)) }],
    };
  });
  const id = `ray${color.replace('#', '')}`;
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: x - size / 2, top: y - size / 2, width: size, height: size }, style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={color} stopOpacity={1} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Path d={path} fill={`url(#${id})`} />
      </Svg>
    </Animated.View>
  );
}

export function Glow({
  clock,
  at,
  x,
  y,
  color,
  size = 320,
  opacity = 0.5,
  pulse = 0,
}: {
  clock: Clock;
  at: number;
  x: number;
  y: number;
  color: string;
  size?: number;
  opacity?: number;
  pulse?: number;
}) {
  const style = useAnimatedStyle(() => {
    const ms = clock.value - at;
    const p = clamp01(ms / 400);
    const beat = pulse ? 1 + 0.08 * Math.sin((Math.max(0, ms) / 1000) * pulse * Math.PI * 2) : 1;
    return { opacity: p * opacity, transform: [{ scale: (0.7 + 0.3 * p) * beat }] };
  });
  const id = `glow${color.replace('#', '')}`;
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: x - size / 2, top: y - size / 2, width: size, height: size }, style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={color} stopOpacity={1} />
            <Stop offset="0.45" stopColor={color} stopOpacity={0.35} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={size / 2} cy={size / 2} r={size / 2} fill={`url(#${id})`} />
      </Svg>
    </Animated.View>
  );
}

export function Vignette({ w, h, color, strength = 0.85 }: { w: number; h: number; color: string; strength?: number }) {
  const id = `vig${color.replace('#', '')}`;
  return (
    <View pointerEvents="none" style={StyleSheet.absoluteFill}>
      <Svg width={w} height={h}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="45%" r="75%">
            <Stop offset="0.45" stopColor={color} stopOpacity={0} />
            <Stop offset="1" stopColor={color} stopOpacity={strength} />
          </RadialGradient>
        </Defs>
        <Rect x={0} y={0} width={w} height={h} fill={`url(#${id})`} />
      </Svg>
    </View>
  );
}

export function Scanlines({ w, h, color, opacity, thick }: { w: number; h: number; color: string; opacity: number; thick: number }) {
  const rows = Array.from({ length: Math.ceil(h / 4) }, (_, i) => i * 4);
  return (
    <Svg pointerEvents="none" width={w} height={h} style={StyleSheet.absoluteFill}>
      {rows.map((y) => (
        <Rect key={y} x={0} y={y} width={w} height={thick} fill={color} fillOpacity={opacity} />
      ))}
    </Svg>
  );
}

export function SweepBand({
  clock,
  w,
  h,
  period,
  size,
  reach,
  peak,
  color,
}: {
  clock: Clock;
  w: number;
  h: number;
  period: number;
  size: number;
  reach: number;
  peak: number;
  color: string;
}) {
  const style = useAnimatedStyle(() => ({ transform: [{ translateY: ((clock.value / period) % 1) * (h + reach) - size }] }));
  const id = `sweep${color.replace('#', '')}`;
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: 0, top: 0, width: w, height: size }, style]}>
      <Svg width={w} height={size}>
        <Defs>
          <LinearGradient id={id} x1="0" y1="0" x2="0" y2="1">
            <Stop offset="0" stopColor={color} stopOpacity={0} />
            <Stop offset={peak} stopColor={color} stopOpacity={0.07} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </LinearGradient>
        </Defs>
        <Rect x={0} y={0} width={w} height={size} fill={`url(#${id})`} />
      </Svg>
    </Animated.View>
  );
}
