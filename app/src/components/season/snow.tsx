import { memo, useMemo } from 'react';
import { View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion } from 'react-native-reanimated';
import Svg, { Path } from 'react-native-svg';
import { useFocusedSafe } from '@/lib/hooks/use-focused-safe';
import { snowClock, useSnowClock } from '@/lib/hooks/use-snow-clock';
import { useScheme } from '@/lib/theme';
import { SEASON } from '@/lib/tokens';
import { CRYSTAL, CRYSTAL_DEPTH, type Flake, flakeAt, SNOW_BOOST, snowLayout, SPECK_DEPTH } from '@/lib/utils/snowfall';

const LIGHT_FALL = 14;
const DEFAULT_SEED = 7;
const CRYSTAL_STROKE = 2;

export const Snow = memo(function Snow({ height, count = LIGHT_FALL, seed = DEFAULT_SEED }: { height: number; count?: number; seed?: number }) {
  const scheme = useScheme();
  const reduced = useReducedMotion();
  const focused = useFocusedSafe();
  const live = !reduced && focused;
  useSnowClock(live);
  const flakes = useMemo(() => snowLayout(count, seed), [count, seed]);
  return (
    <View
      pointerEvents="none"
      accessible={false}
      aria-hidden
      importantForAccessibility="no-hide-descendants"
      accessibilityElementsHidden
      style={{ position: 'absolute', left: 0, right: 0, top: 0, height, overflow: 'hidden' }}>
      {flakes.map((f, i) => (
        <FlakeView key={i} f={f} height={height} scheme={scheme} live={live} />
      ))}
    </View>
  );
});

const FlakeView = memo(function FlakeView({ f, height, scheme, live }: { f: Flake; height: number; scheme: 'dark' | 'light'; live: boolean }) {
  const tone = SEASON.snow[scheme];
  const boost = SNOW_BOOST[scheme];
  const style = useAnimatedStyle(() => {
    const p = flakeAt(f, live ? snowClock.value : 0, height);
    return { opacity: Math.min(1, p.opacity * boost), transform: [{ translateX: p.x }, { translateY: p.y }, { rotate: `${p.turn}deg` }] };
  });
  const size = f.r * 2;
  const at = { position: 'absolute', top: 0, left: `${f.left * 100}%`, width: size, height: size } as const;
  if (f.depth === CRYSTAL_DEPTH)
    return (
      <Animated.View style={[at, style]}>
        <Svg width={size} height={size} viewBox="0 0 24 24">
          <Path d={CRYSTAL} fill="none" stroke={tone.crystal} strokeWidth={CRYSTAL_STROKE} strokeLinecap="round" strokeLinejoin="round" />
        </Svg>
      </Animated.View>
    );
  return <Animated.View style={[at, { borderRadius: f.r, backgroundColor: f.depth === SPECK_DEPTH ? tone.speck : tone.flake }, style]} />;
});
