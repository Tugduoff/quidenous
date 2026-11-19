import { useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, Path, RadialGradient, Rect, Stop } from 'react-native-svg';

import { wedgePath } from '@/lib/utils/svg-paths';

import { burst, type Clock, Particles, sparks } from '@/components/celebration/engine';
import { Flash, Shockwave } from '@/components/celebration/effects';
import { centred } from '@/components/celebration/layout';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

const BEAM_HALF_WIDTH = (20 * Math.PI) / 180;

const RED = '#FF1E3C';
const BLUE = '#1E5BFF';

function Beams({ clock, x, y, size }: { clock: Clock; x: number; y: number; size: number }) {
  const path = (a: number) => wedgePath(size / 2, a, BEAM_HALF_WIDTH);
  const style = useAnimatedStyle(() => ({
    opacity: interpolate(clock.value, [100, 400], [0, 1], Extrapolation.CLAMP),
    transform: [{ rotate: `${(clock.value / 1300) * 360}deg` }],
  }));
  return (
    <Animated.View pointerEvents="none" style={[centred(x, y, size, size), style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id="beamR" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={RED} stopOpacity={0.75} />
            <Stop offset="1" stopColor={RED} stopOpacity={0} />
          </RadialGradient>
          <RadialGradient id="beamB" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={BLUE} stopOpacity={0.75} />
            <Stop offset="1" stopColor={BLUE} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Path d={path(0)} fill="url(#beamR)" />
        <Path d={path(Math.PI)} fill="url(#beamB)" />
      </Svg>
    </Animated.View>
  );
}

function Wash({ clock, color, phase, peak, period = 520 }: { clock: Clock; color: string; phase: 0 | 1; peak: number; period?: number }) {
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    if (c < 150) return { opacity: 0 };
    const s = Math.sin(((c - 150) / period) * Math.PI * 2);
    const v = phase === 0 ? Math.max(0, s) : Math.max(0, -s);
    return { opacity: v * v * v * peak };
  });
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: color }, style]} />;
}

function LightBar({ clock, x, y }: { clock: Clock; x: number; y: number }) {
  return (
    <>
      <Halo clock={clock} x={x - 30} y={y} color={RED} phase={0} />
      <Halo clock={clock} x={x + 30} y={y} color={BLUE} phase={1} />
      <View pointerEvents="none" style={centred(x, y + 6, 120, 50)}>
        <Svg width={120} height={50} viewBox="0 0 120 50">
          <Path d="M8,30 C8,12 22,6 42,6 L58,6 L58,30 Z" fill={RED} />
          <Path d="M62,6 L78,6 C98,6 112,12 112,30 L62,30 Z" fill={BLUE} />
          <Path d="M22,12 C26,9 32,8 40,8" stroke="#FFFFFF" strokeOpacity={0.7} strokeWidth={3} strokeLinecap="round" fill="none" />
          <Path d="M80,8 C88,8 94,9 98,12" stroke="#FFFFFF" strokeOpacity={0.7} strokeWidth={3} strokeLinecap="round" fill="none" />
          <Rect x={2} y={30} width={116} height={14} rx={5} fill="#2A2D3A" />
        </Svg>
      </View>
    </>
  );
}

function Halo({ clock, x, y, color, phase }: { clock: Clock; x: number; y: number; color: string; phase: 0 | 1 }) {
  const size = 260;
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const s = Math.sin(((c - 150) / 520) * Math.PI * 2);
    const v = c < 150 ? 0 : phase === 0 ? Math.max(0, s) : Math.max(0, -s);
    return { opacity: 0.25 + v * 0.75, transform: [{ scale: 0.8 + v * 0.3 }] };
  });
  const id = `halo${phase}`;
  return (
    <Animated.View pointerEvents="none" style={[centred(x, y, size, size), style]}>
      <Svg width={size} height={size}>
        <Defs>
          <RadialGradient id={id} cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor={color} stopOpacity={0.9} />
            <Stop offset="0.35" stopColor={color} stopOpacity={0.35} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={size / 2} cy={size / 2} r={size / 2} fill={`url(#${id})`} />
      </Svg>
    </Animated.View>
  );
}

export const siren: SceneFactory = (m, t) => {
  const slam = 1100;
  return {
    duration: 3900,
    bg: '#07070D',
    dim: 0.97,
    impacts: [{ at: slam, amp: 8 }],
    Render: function Siren({ clock, w, h, rand, cx, cy }: SceneProps) {
      const by = Math.max(70, cy - 210);
      const glints = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 16,
            speed: [500, 900],
            g: 300,
            k: 2.4,
            life: [500, 800],
            look: sparks([RED, '#FFFFFF'], 3),
          }),
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam + 20,
            count: 16,
            speed: [500, 900],
            g: 300,
            k: 2.4,
            life: [500, 800],
            look: sparks([BLUE, '#FFFFFF'], 3),
          }),
        ],
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Beams clock={clock} x={cx} y={by} size={Math.max(w, h) * 2.2} />
          <Wash clock={clock} color={RED} phase={0} peak={0.3} />
          <Wash clock={clock} color={BLUE} phase={1} peak={0.3} />
          <LightBar clock={clock} x={cx} y={by} />
          <Flash clock={clock} at={slam} color="#FFFFFF" peak={0.45} fall={300} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FFFFFF" size={480} width={6} />
          <Particles clock={clock} list={glints} />
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 250} cy={cy} size={size} color="#FFFFFF" glow="#FFFFFF66" ink="#FFFFFF" sub="#C8CCDA" />
        </>
      );
    },
  };
};
