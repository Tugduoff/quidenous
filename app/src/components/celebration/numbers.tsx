import { useMemo } from 'react';
import { Text, type TextStyle, View } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';

import { backOut, clamp01 } from '@/lib/utils/easing';

import type { Clock } from '@/components/celebration/engine';

export const glowText = (color: string, radius = 24) => ({ textShadowColor: color, textShadowRadius: radius, textShadowOffset: { width: 0, height: 0 } });

const ROLL_OVERSHOOT = 1.4;

function RollDigit({
  clock,
  digit,
  start,
  dur,
  size,
  color,
  font,
  spins,
}: {
  clock: Clock;
  digit: number;
  start: number;
  dur: number;
  size: number;
  color: string;
  font: string;
  spins: number;
}) {
  const lineH = Math.round(size * 1.02);
  const strip = useMemo(() => {
    const out: number[] = [];
    for (let i = 0; i < spins * 10 + digit + 1; i++) out.push(i % 10);
    return out;
  }, [digit, spins]);
  const end = (strip.length - 1) * lineH;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - start) / dur);
    return { transform: [{ translateY: -backOut(p, ROLL_OVERSHOOT) * end }] };
  });
  const text: TextStyle = {
    fontFamily: font,
    fontSize: size,
    lineHeight: lineH,
    height: lineH,
    color,
    textAlign: 'center',
    letterSpacing: -2,
    includeFontPadding: false,
  };
  return (
    <View style={{ height: lineH, overflow: 'hidden' }}>
      <Animated.View style={style}>
        {strip.map((n, i) => (
          <Text key={i} style={text}>
            {n}
          </Text>
        ))}
      </Animated.View>
    </View>
  );
}

export function RollNumber({
  clock,
  value,
  at,
  size,
  color,
  font,
  dur = 900,
  stagger = 140,
}: {
  clock: Clock;
  value: number;
  at: number;
  size: number;
  color: string;
  font: string;
  dur?: number;
  stagger?: number;
}) {
  const digits = String(value).split('').map(Number);
  return (
    <View style={{ flexDirection: 'row' }}>
      {digits.map((d, i) => (
        <RollDigit
          key={i}
          clock={clock}
          digit={d}
          start={at + i * stagger}
          dur={dur}
          size={size}
          color={color}
          font={font}
          spins={1 + (digits.length - i > 1 ? 1 : 0)}
        />
      ))}
    </View>
  );
}

export function BigNumber({
  value,
  size,
  color,
  font,
  glow,
  style,
}: {
  value: string | number;
  size: number;
  color: string;
  font: string;
  glow?: string;
  style?: TextStyle;
}) {
  return (
    <Text
      style={[
        {
          fontFamily: font,
          fontSize: size,
          lineHeight: Math.round(size * 1.04),
          color,
          letterSpacing: -3,
          textAlign: 'center',
          includeFontPadding: false,
        },
        glow ? glowText(glow) : null,
        style,
      ]}>
      {value}
    </Text>
  );
}
