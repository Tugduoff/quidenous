import { useMemo } from 'react';
import { View } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, Ellipse, LinearGradient, Path, RadialGradient, Rect, Stop } from 'react-native-svg';

import { clamp01, easeOutCubic } from '@/lib/utils/easing';

import { burst, type Clock, type Particle, Particles, sparks } from '@/components/celebration/engine';
import { Glow } from '@/components/celebration/effects';
import { ABS } from '@/components/celebration/layout';
import { Pop } from '@/components/celebration/motion';
import { BigNumber } from '@/components/celebration/numbers';
import { GOLD } from '@/components/celebration/palettes';
import { Headline, numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';
import { StarField } from '@/components/celebration/stars';

export const night: SceneFactory = (m, t) => {
  const reveal = 1500;
  return {
    duration: 6400,
    bg: '#070B1E',
    dim: 0.98,
    impacts: [{ at: reveal + 250, amp: 2 }],
    Render: function Night({ clock, w, h, rand, cx, cy }: SceneProps) {
      const lanterns = useMemo<Particle[]>(
        () =>
          Array.from({ length: 15 }, () => {
            const s = rand.range(0.55, 1.25);
            const lw = Math.round(52 * s);
            const lh = Math.round(52 * s);
            return {
              x: rand.range(20, w - 20),
              y: h + 30 + rand.range(0, 100),
              vx: rand.range(-8, 8),
              vy: -rand.range(80, 140) * (0.6 + s * 0.4),
              k: 0.001,
              at: rand.range(-1200, 2600),
              life: 9000,
              fadeIn: 700,
              sway: rand.range(6, 14),
              swayHz: rand.range(0.2, 0.45),
              rot: rand.range(-6, 6),
              w: lw,
              h: lh,
              color: '#FFB547',
              shape: 'node',
              node: <Lantern w={lw} h={lh} />,
            } as Particle;
          }),
        [rand, w, h],
      );
      const shooting = useMemo<Particle[]>(
        () =>
          [900, 2700, 4400].map((at) => ({
            x: rand.range(w * 0.05, w * 0.5),
            y: rand.range(h * 0.04, h * 0.2),
            vx: 760,
            vy: 300,
            k: 0.001,
            at,
            life: 600,
            fadeIn: 60,
            fadeOut: 380,
            w: 80,
            h: 2,
            color: '#FFFFFF',
            shape: 'streak',
          })),
        [rand, w, h],
      );
      const sparkle = useMemo(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: reveal + 200,
            count: 22,
            speed: [120, 380],
            g: 60,
            k: 2,
            life: [1200, 1900],
            fadeOut: 700,
            look: sparks(GOLD, 2.5),
          }),
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Sky w={w} h={h} />
          <StarField clock={clock} rand={rand} w={w} h={h} count={96} maxY={0.75} />
          <Moon clock={clock} w={w} h={h} />
          <Particles clock={clock} list={shooting} />
          <Glow clock={clock} at={reveal} x={cx} y={cy} color="#FFB547" size={360} opacity={0.32} pulse={0.5} />
          <Particles clock={clock} list={lanterns} />
          <Particles clock={clock} list={sparkle} />
          <Headline clock={clock} m={m} t={t} at={reveal + 450} cy={cy} box={size * 1.1} ink="#FFF6D8" sub="#B9B3D9">
            <Pop clock={clock} at={reveal}>
              <BigNumber value={m.n} size={size} color="#FFE9A8" font={t.font.display} glow="#FFB547" />
            </Pop>
          </Headline>
        </>
      );
    },
  };
};

function Sky({ w, h }: { w: number; h: number }) {
  return (
    <Svg pointerEvents="none" width={w} height={h} style={ABS}>
      <Defs>
        <LinearGradient id="sky1001" x1="0" y1="0" x2="0" y2="1">
          <Stop offset="0" stopColor="#050818" />
          <Stop offset="0.65" stopColor="#141A44" />
          <Stop offset="1" stopColor="#3A2350" />
        </LinearGradient>
      </Defs>
      <Rect x={0} y={0} width={w} height={h} fill="url(#sky1001)" />
    </Svg>
  );
}

function Moon({ clock, w, h }: { clock: Clock; w: number; h: number }) {
  const S = 150;
  const [x, y0, y1] = [w * 0.74, h * 0.34, h * 0.13];
  const anim = useAnimatedStyle(() => {
    const p = easeOutCubic(clamp01(clock.value / 2600));
    return {
      opacity: clamp01(clock.value / 700),
      transform: [{ translateX: x - S / 2 }, { translateY: y0 + (y1 - y0) * p - S / 2 }, { rotate: `${-18 + 10 * p}deg` }],
    };
  });
  const R = 30;
  const c = S / 2;
  return (
    <Animated.View pointerEvents="none" style={[ABS, { width: S, height: S }, anim]}>
      <Svg width={S} height={S}>
        <Defs>
          <RadialGradient id="moonHalo" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#FFE9A8" stopOpacity={0.45} />
            <Stop offset="0.4" stopColor="#FFE9A8" stopOpacity={0.12} />
            <Stop offset="1" stopColor="#FFE9A8" stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={c} cy={c} r={c} fill="url(#moonHalo)" />
        <Path d={`M${c},${c - R} A${R},${R} 0 1 0 ${c},${c + R} A${R * 1.3},${R * 1.3} 0 0 1 ${c},${c - R} Z`} fill="#FFE9A8" />
      </Svg>
    </Animated.View>
  );
}

function Lantern({ w, h }: { w: number; h: number }) {
  const lw = w / 2;
  const lh = (lw * 36) / 26;
  return (
    <View style={{ width: w, height: h, alignItems: 'center', justifyContent: 'center' }}>
      <Svg width={w} height={h} style={ABS}>
        <Defs>
          <RadialGradient id="lanternHalo" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#FFB547" stopOpacity={0.35} />
            <Stop offset="1" stopColor="#FFB547" stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Circle cx={w / 2} cy={h / 2} r={Math.min(w, h) / 2} fill="url(#lanternHalo)" />
      </Svg>
      <Svg width={lw} height={lh} viewBox="0 0 26 36">
        <Defs>
          <RadialGradient id="lantern" cx="50%" cy="85%" r="80%">
            <Stop offset="0" stopColor="#FFF4C2" />
            <Stop offset="0.45" stopColor="#FFB547" />
            <Stop offset="1" stopColor="#D9541E" />
          </RadialGradient>
        </Defs>
        <Path d="M3 2 Q13 -1 23 2 L20 30 Q13 32 6 30 Z" fill="url(#lantern)" />
        <Ellipse cx={13} cy={31} rx={4} ry={2.2} fill="#FFF4C2" />
      </Svg>
    </View>
  );
}
