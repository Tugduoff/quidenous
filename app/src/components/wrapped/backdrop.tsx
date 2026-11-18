import { useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import Svg, { Circle, Defs, LinearGradient, Rect, Stop } from 'react-native-svg';
import { useTheme } from '@/lib/theme';
import type { WrappedCard } from '@/lib/api/wrapped';
import { colorsOf } from '@/lib/utils/wrapped';
import { INK } from '@/components/wrapped/constants';

/** The card's colours, full screen, with a few big shapes for rhythm (where they sit changes from card to card). */
export function Backdrop({ card, index, year }: { card: WrappedCard; index: number; year: number }) {
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
