import { type ReactNode, useMemo } from 'react';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, Path, RadialGradient, Stop } from 'react-native-svg';

import { clamp01, easeOutCubic } from '@/lib/utils/easing';

import { type Clock, type Particle, Particles } from '@/components/celebration/engine';
import { Flash, Glow, Shockwave } from '@/components/celebration/effects';
import { ABS } from '@/components/celebration/layout';
import { Orbiter } from '@/components/celebration/motion';
import { BigNumber } from '@/components/celebration/numbers';
import { Headline, numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';
import { StarField } from '@/components/celebration/stars';

const SPIN = 0.45;
const ARM_TWIST = 3.4;
const TILT = 0.55;
const TILT_ROT = -0.32;

function Arms({ clock, cx, cy, R, color }: { clock: Clock; cx: number; cy: number; R: number; color: string }) {
  const S = R * 2 + 40;
  const blobs = useMemo(
    () =>
      [0, 1, 2].flatMap((arm) =>
        Array.from({ length: 36 }, (_, i) => {
          const u = i / 35;
          const a = (arm / 3) * Math.PI * 2 + u * ARM_TWIST;
          const r = 14 + u * R;
          return { x: S / 2 + r * Math.cos(a), y: S / 2 + r * Math.sin(a), r: 34 - 16 * u, accent: i % 9 === 4 && u < 0.7 };
        }),
      ),
    [R, S],
  );
  const anim = useAnimatedStyle(() => {
    const ms = clock.value;
    const b = easeOutCubic(clamp01(ms / 1900));
    return {
      opacity: b,
      transform: [
        { rotate: `${(TILT_ROT * 180) / Math.PI}deg` },
        { scaleY: TILT },
        { rotate: `${((SPIN * ms) / 1000) * (180 / Math.PI)}deg` },
        { scale: 0.1 + 0.9 * b },
      ],
    };
  });
  return (
    <Animated.View pointerEvents="none" style={[{ position: 'absolute', left: cx - S / 2, top: cy - S / 2, width: S, height: S }, anim]}>
      <Svg width={S} height={S}>
        <Defs>
          <RadialGradient id="dust42" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#A98BFF" stopOpacity={0.24} />
            <Stop offset="1" stopColor="#A98BFF" stopOpacity={0} />
          </RadialGradient>
          <RadialGradient id="dustAcc42" cx="50%" cy="50%" r="50%">
            <Stop offset="0" stopColor="#FFFFFF" stopOpacity={0.18} />
            <Stop offset="1" stopColor={color} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        {blobs.map((b, i) => (
          <Circle key={i} cx={b.x} cy={b.y} r={b.r} fill={b.accent ? 'url(#dustAcc42)' : 'url(#dust42)'} />
        ))}
      </Svg>
    </Animated.View>
  );
}

export const galaxy: SceneFactory = (m, t) => {
  const birth = 1700;
  const colors = [t.acc, '#B48CFF', '#5AC8FA', '#FFFFFF', '#C2A3FF', '#7FB2FF'];
  return {
    duration: 6000,
    bg: '#05040F',
    dim: 0.97,
    impacts: [{ at: birth + 380, amp: 4 }],
    Render: function Galaxy({ clock, w, h, rand, cx, cy }: SceneProps) {
      const R = Math.min(w * 0.5, 210);
      const dots = useMemo(() => {
        const arms = 3;
        return Array.from({ length: 120 }, (_, i) => {
          const arm = i % arms;
          const u = Math.pow(rand.f(), 0.8);
          const r1 = 14 + u * R + rand.range(-8, 8);
          const a = (arm / arms) * Math.PI * 2 + u * ARM_TWIST + rand.range(-0.28, 0.28) * (1 - u * 0.4);
          const spin = SPIN + 0.15 * (50 / (r1 + 50));
          const core = u < 0.18;
          return {
            o: { r0: r1 * 0.08, r1, a, spin, at: rand.range(0, 900) + u * 500, dur: 1500, size: core ? rand.range(3.5, 6) : rand.range(2, 4.6) },
            color: core ? rand.pick(['#FFFFFF', '#F2E8FF', t.acc]) : rand.pick(colors),
          };
        });
      }, [rand, R]);
      const size = numberSize(m.n);
      return (
        <>
          <StarField clock={clock} rand={rand} w={w} h={h} count={90} />
          <Glow clock={clock} at={0} x={cx} y={cy} color="#6A3FD0" size={R * 2.6} opacity={0.55} pulse={0.4} />
          <Glow clock={clock} at={300} x={cx} y={cy} color="#B48CFF" size={R * 1.1} opacity={0.6} pulse={0.7} />
          <Arms clock={clock} cx={cx} cy={cy} R={R} color={t.acc} />
          {dots.map((d, i) => (
            <Orbiter
              key={i}
              clock={clock}
              o={d.o}
              cx={cx}
              cy={cy}
              tilt={TILT}
              tiltRot={TILT_ROT}
              style={{ borderRadius: d.o.size / 2, backgroundColor: d.color }}
            />
          ))}
          <Glow clock={clock} at={birth} x={cx} y={cy} color={t.acc} size={260} opacity={0.4} pulse={0.9} />
          <Flash clock={clock} at={birth + 380} color="#C2A3FF" peak={0.25} fall={600} />
          <Shockwave clock={clock} at={birth + 380} x={cx} y={cy} color="#C2A3FF" size={460} width={3} dur={900} />
          <Rocket clock={clock} from={3000} to={5600} w={w} h={h} />
          <Headline clock={clock} m={m} t={t} at={birth + 700} cy={cy} box={size * 1.1} ink="#F4F0FF" sub="#A9A3C9">
            <Emerge clock={clock} at={birth}>
              <BigNumber value={m.n} size={size} color="#FFFFFF" font={t.font.display} glow={t.acc} />
            </Emerge>
          </Headline>
        </>
      );
    },
  };
};

function Emerge({ clock, at, children }: { clock: Clock; at: number; children: ReactNode }) {
  const anim = useAnimatedStyle(() => {
    const c = clock.value;
    const s = interpolate(c, [at, at + 380, at + 520, at + 680], [0.05, 1.12, 0.97, 1], Extrapolation.CLAMP);
    const r = interpolate(c, [at, at + 380], [-100, 0], Extrapolation.CLAMP);
    return { opacity: interpolate(c, [at, at + 160], [0, 1], Extrapolation.CLAMP), transform: [{ rotate: `${r}deg` }, { scale: s }] };
  });
  return <Animated.View style={anim}>{children}</Animated.View>;
}

function Rocket({ clock, from, to, w, h }: { clock: Clock; from: number; to: number; w: number; h: number }) {
  const [x0, y0, x1, y1] = [-50, h * 0.8, w + 50, h * 0.66];
  const angle = (Math.atan2(y1 - y0, x1 - x0) * 180) / Math.PI;
  const anim = useAnimatedStyle(() => {
    const p = clamp01((clock.value - from) / (to - from));
    const on = clock.value >= from && p < 1;
    const bob = Math.sin(p * Math.PI * 6) * 4;
    return {
      opacity: on ? 1 : 0,
      transform: [{ translateX: x0 + (x1 - x0) * p - 22 }, { translateY: y0 + (y1 - y0) * p - 11 + bob }, { rotate: `${angle}deg` }],
    };
  });
  const trail = useMemo<Particle[]>(
    () =>
      Array.from({ length: 16 }, (_, i) => {
        const p = (i + 0.5) / 16;
        return {
          x: x0 + (x1 - x0) * p - 18,
          y: y0 + (y1 - y0) * p + 2,
          vx: -40,
          vy: 15,
          k: 1,
          at: from + p * (to - from),
          life: 700,
          fadeOut: 650,
          grow: [1, 0.3],
          w: 5,
          h: 5,
          color: i % 2 ? '#FFB547' : '#FFF1B8',
          shape: 'circle',
        } as Particle;
      }),
    [x0, y0, x1, y1, from, to],
  );
  return (
    <>
      <Particles clock={clock} list={trail} />
      <Animated.View pointerEvents="none" style={[ABS, { width: 44, height: 22 }, anim]}>
        <Svg width={44} height={22} viewBox="0 0 44 22">
          <Path d="M8 11 L2 4 L12 7 Z M8 11 L2 18 L12 15 Z" fill="#FF4FA3" />
          <Path d="M8 6 Q30 2 42 11 Q30 20 8 16 Z" fill="#EDEAF7" />
          <Circle cx={27} cy={11} r={3.2} fill="#5AC8FA" stroke="#2B2350" strokeWidth={1.4} />
          <Path d="M8 8 L4 11 L8 14 Z" fill="#FFB547" />
        </Svg>
      </Animated.View>
    </>
  );
}
