import { Pressable, StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion } from 'react-native-reanimated';
import { MascotBand } from '@/components/mascot/band';
import { useSeasonSkin } from '@/components/season/season-scope';
import { Snow } from '@/components/season/snow';
import { Snowdrift } from '@/components/season/snowdrift';
import { Logo } from '@/components/ui/text';
import { useSeasonSplash, useSplashTimeline } from '@/lib/hooks/use-season-splash';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import type { SeasonId } from '@/lib/utils/season';

const OVER_THE_APP = 50;
const SPLASH_FLAKES = 20;
const SPLASH_SEED = 11;
const BAND_MAX = 340;
const BAND_MARGIN = 48;
const SETTLE_FROM_SCALE = 0.94;
const RISE_FROM_PX = 12;

export function SeasonSplash() {
  const shown = useSeasonSplash(useSeasonSkin());
  return shown ? <Splash id={shown} /> : null;
}

function Splash({ id }: { id: SeasonId }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const { width, height } = useWindowDimensions();
  const { show, settle, leaving, gone, skip } = useSplashTimeline(reduced);

  const fadeStyle = useAnimatedStyle(() => ({ opacity: show.value }));
  const markStyle = useAnimatedStyle(() => ({ transform: [{ scale: SETTLE_FROM_SCALE + (1 - SETTLE_FROM_SCALE) * settle.value }] }));
  const bandStyle = useAnimatedStyle(() => ({ opacity: Math.min(1, settle.value * 1.4), transform: [{ translateY: (1 - settle.value) * RISE_FROM_PX }] }));

  const band = Math.round(Math.min(BAND_MAX, width - BAND_MARGIN));
  if (gone) return null;
  return (
    <Animated.View
      pointerEvents={leaving ? 'none' : 'auto'}
      accessible={false}
      aria-hidden
      importantForAccessibility="no-hide-descendants"
      accessibilityElementsHidden
      style={[StyleSheet.absoluteFill, { zIndex: OVER_THE_APP, backgroundColor: t.bg }, fadeStyle]}>
      <Pressable onPress={skip} style={StyleSheet.absoluteFill} accessible={false}>
        <Snow height={height} count={SPLASH_FLAKES} seed={SPLASH_SEED} />
        <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
          <Animated.View style={[{ alignItems: 'center', gap: 8 }, markStyle]}>
            <Logo size={40} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.mut }}>{tr(`season.greeting.${id}`)}</Text>
          </Animated.View>
          <Animated.View style={[{ width: band, marginTop: 34 }, bandStyle]}>
            <Snowdrift width={band} part="hill" />
            <MascotBand hello width={band} />
            <Snowdrift width={band} part="bank" />
          </Animated.View>
        </View>
      </Pressable>
    </Animated.View>
  );
}
