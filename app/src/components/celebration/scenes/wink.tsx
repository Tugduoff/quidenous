import { type ReactNode, useMemo } from 'react';
import { View } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, Ellipse, Path, RadialGradient, Stop } from 'react-native-svg';

import { burst, cannons, type Clock, paper, Particles } from '@/components/celebration/engine';
import { Glow, Shockwave } from '@/components/celebration/effects';
import { Pop, Slam } from '@/components/celebration/motion';
import { BigNumber } from '@/components/celebration/numbers';
import { Headline, numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

const PINKS = ['#FF4FA3', '#FF8FB1', '#FFB38A', '#FFD1DC', '#FFC94D'];

export const wink: SceneFactory = (m, t) => {
  const slam = 650;
  const turn = 2350;
  return {
    duration: 5400,
    bg: t.bg,
    impacts: [
      { at: slam, amp: 6 },
      { at: turn + 520, amp: 4 },
    ],
    Render: function Wink({ clock, w, h, rand, cx, cy }: SceneProps) {
      const fy = Math.max(cy - 200, 120);
      const list = useMemo(
        () => [
          ...burst(rand, { x: cx, y: cy, at: slam, count: 26, speed: [350, 800], g: 900, k: 1.8, life: [1200, 1700], look: paper(PINKS) }),
          ...burst(rand, { x: cx, y: cy, at: turn + 520, count: 40, speed: [450, 1000], g: 900, k: 1.7, life: [1500, 2200], look: paper(PINKS) }),
          ...cannons(rand, {
            w,
            y: h * 0.7,
            at: turn + 560,
            count: 14,
            dir: -55,
            spread: 30,
            speed: [800, 1300],
            g: 900,
            k: 1.3,
            life: [1800, 2300],
            look: paper(PINKS),
          }),
        ],
        [rand, w, h, cx, cy],
      );
      const size = numberSize(m.n) + 20;
      return (
        <>
          <Glow clock={clock} at={turn + 520} x={cx} y={cy} color="#FF4FA3" size={360} opacity={0.3} pulse={0.6} />
          <Shockwave clock={clock} at={turn + 520} x={cx} y={cy} color="#FF8FB1" size={420} width={5} />
          <Face clock={clock} x={cx} y={fy} size={160} winks={[1250, 3200]} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={turn + 650} cy={cy} box={size * 1.1}>
            <Slam clock={clock} at={slam}>
              <Turn clock={clock} at={turn}>
                <BigNumber value={m.n} size={size} color="#FF4FA3" font={t.font.display} />
              </Turn>
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

function Turn({ clock, at, children }: { clock: Clock; at: number; children: ReactNode }) {
  const anim = useAnimatedStyle(() => {
    const r = interpolate(clock.value, [at, at + 140, at + 460, at + 560, at + 680], [0, -12, 192, 176, 180], Extrapolation.CLAMP);
    const s = interpolate(clock.value, [at, at + 260, at + 520, at + 680], [1, 0.86, 1.08, 1], Extrapolation.CLAMP);
    return { transform: [{ rotate: `${r}deg` }, { scale: s }] };
  });
  return <Animated.View style={anim}>{children}</Animated.View>;
}

function Face({ clock, x, y, size, winks }: { clock: Clock; x: number; y: number; size: number; winks: number[] }) {
  const s = size / 170;
  const [w1, w2] = winks;
  const wink = (c: number) => {
    'worklet';
    const a = interpolate(c, [w1, w1 + 90, w1 + 700, w1 + 800], [0, 1, 1, 0], Extrapolation.CLAMP);
    const b = interpolate(c, [w2, w2 + 90, w2 + 520, w2 + 620], [0, 1, 1, 0], Extrapolation.CLAMP);
    return Math.max(a, b);
  };
  const head = useAnimatedStyle(() => ({ transform: [{ rotate: `${-9 * wink(clock.value)}deg` }] }));
  const open = useAnimatedStyle(() => ({ transform: [{ scaleY: 1 - 0.92 * wink(clock.value) }], opacity: 1 - wink(clock.value) * 0.9 }));
  const shut = useAnimatedStyle(() => ({ opacity: wink(clock.value) }));
  const brow = useAnimatedStyle(() => ({ transform: [{ translateY: -10 * s * wink(clock.value) }, { rotate: `${-12 * wink(clock.value)}deg` }] }));
  const ink = '#2B1A0E';
  return (
    <View pointerEvents="none" style={{ position: 'absolute', left: x - size / 2, top: y - size / 2, width: size, height: size }}>
      <Pop clock={clock} at={120}>
        <Animated.View style={[{ width: size, height: size }, head]}>
          <Svg width={size} height={size} viewBox="0 0 170 170">
            <Defs>
              <RadialGradient id="face69" cx="40%" cy="35%" r="70%">
                <Stop offset="0" stopColor="#FFE07A" />
                <Stop offset="1" stopColor="#FFB938" />
              </RadialGradient>
            </Defs>
            <Circle cx={85} cy={85} r={82} fill="url(#face69)" />
            <Circle cx={40} cy={108} r={14} fill="#FF7FA8" opacity={0.45} />
            <Circle cx={140} cy={114} r={13} fill="#FF7FA8" opacity={0.45} />
            <Path d="M48 110 Q92 142 132 100" stroke={ink} strokeWidth={9} fill="none" strokeLinecap="round" />
            <Path d="M100 42 Q116 36 130 44" stroke={ink} strokeWidth={7} fill="none" strokeLinecap="round" />
          </Svg>
          <View style={{ position: 'absolute', left: 44 * s, top: 56 * s, width: 24 * s, height: 32 * s }}>
            <Svg width={24 * s} height={32 * s} viewBox="0 0 24 32">
              <Ellipse cx={12} cy={16} rx={11} ry={15} fill={ink} />
              <Circle cx={15} cy={10} r={3.5} fill="#FFFFFF" />
            </Svg>
          </View>
          <Animated.View style={[{ position: 'absolute', left: 34 * s, top: 30 * s, width: 44 * s, height: 20 * s }, brow]}>
            <Svg width={44 * s} height={20 * s} viewBox="0 0 44 20">
              <Path d="M4 14 Q22 2 40 10" stroke={ink} strokeWidth={7} fill="none" strokeLinecap="round" />
            </Svg>
          </Animated.View>
          <Animated.View style={[{ position: 'absolute', left: 104 * s, top: 56 * s, width: 24 * s, height: 32 * s }, open]}>
            <Svg width={24 * s} height={32 * s} viewBox="0 0 24 32">
              <Ellipse cx={12} cy={16} rx={11} ry={15} fill={ink} />
              <Circle cx={15} cy={10} r={3.5} fill="#FFFFFF" />
            </Svg>
          </Animated.View>
          <Animated.View style={[{ position: 'absolute', left: 98 * s, top: 62 * s, width: 36 * s, height: 22 * s }, shut]}>
            <Svg width={36 * s} height={22 * s} viewBox="0 0 36 22">
              <Path d="M4 14 Q18 2 32 14" stroke={ink} strokeWidth={7} fill="none" strokeLinecap="round" />
            </Svg>
          </Animated.View>
        </Animated.View>
      </Pop>
    </View>
  );
}
