import { memo } from 'react';
import { View } from 'react-native';
import Svg, { Circle, G, Path } from 'react-native-svg';
import { SEASON } from '@/lib/tokens';
import { darken, MASCOT } from '@/lib/utils/mascot';

const WARM_RED_OVER_GREEN = 1.15;

function isWarm(hex: string) {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return r >= g * WARM_RED_OVER_GREEN && r >= b;
}

const woolFor = (mark: string) => (isWarm(mark) ? MASCOT.bodies.pave.wool : MASCOT.bodies.zeste.wool);

const HAT_WIDTH_PER_FONT_PX = 0.62;
const VIEW_W = 60;
const VIEW_H = 56;

export const LogoHat = memo(function LogoHat({ size, mark }: { size: number; mark: string }) {
  const knit = woolFor(mark);
  const shade = darken(knit);
  const ink = MASCOT.ink;
  const w = size * HAT_WIDTH_PER_FONT_PX;
  const h = (w * VIEW_H) / VIEW_W;
  return (
    <View
      pointerEvents="none"
      accessible={false}
      aria-hidden
      importantForAccessibility="no-hide-descendants"
      accessibilityElementsHidden
      style={{ position: 'absolute', right: -size * 0.1, top: -size * 0.15, width: w, height: h }}>
      <Svg width={w} height={h} viewBox={`0 0 ${VIEW_W} ${VIEW_H}`}>
        <G transform="rotate(-14 30 46)">
          <Path d="M11.5 38C11.5 22 19 14 30 14C41 14 48.5 22 48.5 38Z" fill={knit} stroke={ink} strokeWidth={4} strokeLinejoin="round" />
          <Path d="M33.5 16.6C41 18 45.8 24 45.8 37H40.6C40.6 27 38.4 21 33.5 16.6Z" fill={shade} />
          <Path d="M8 36Q30 32.5 52 36Q54 41 52 46Q30 49.5 8 46Q6 41 8 36Z" fill={knit} stroke={ink} strokeWidth={3.6} strokeLinejoin="round" />
          <Path d="M17 38.2V44.2M25.6 37.4V45.2M34.4 37.4V45.2M43 38.2V44.2" fill="none" stroke={shade} strokeWidth={2.4} strokeLinecap="round" />
          <Circle cx={30} cy={10} r={6.6} fill={SEASON.pompom} stroke={ink} strokeWidth={3.6} />
        </G>
      </Svg>
    </View>
  );
});
