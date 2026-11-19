import { useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Path, Rect } from 'react-native-svg';

import { burst, type Clock, paper, type Particle, Particles } from '@/components/celebration/engine';
import { Flash, Rays, Shockwave } from '@/components/celebration/effects';
import { Keyed, Slam } from '@/components/celebration/motion';
import { BigNumber } from '@/components/celebration/numbers';
import { party } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function Rocket() {
  return (
    <Svg width={60} height={130} viewBox="0 0 60 130">
      <Path d="M14,78 L2,104 L16,100 Z" fill="#FF4F5E" />
      <Path d="M46,78 L58,104 L44,100 Z" fill="#FF4F5E" />
      <Path d="M30,2 C46,16 50,44 46,100 L14,100 C10,44 14,16 30,2 Z" fill="#F2F4FA" />
      <Path d="M30,2 C38,9 43,19 45,30 L15,30 C17,19 22,9 30,2 Z" fill="#FF4F5E" />
      <Path d="M34,10 C39,20 41,44 39,92" stroke="#00000018" strokeWidth={5} fill="none" />
      <Circle cx={30} cy={52} r={9} fill="#2B3A67" stroke="#C9D2EA" strokeWidth={3} />
      <Circle cx={27} cy={49} r={2.5} fill="#FFFFFF" fillOpacity={0.7} />
      <Rect x={18} y={100} width={24} height={8} rx={2} fill="#7A8299" />
    </Svg>
  );
}

function Flame({ clock }: { clock: Clock }) {
  const a = useAnimatedStyle(() => {
    const c = clock.value;
    const f = 1 + 0.22 * Math.sin(c * 0.07) + 0.12 * Math.sin(c * 0.19);
    return { transform: [{ translateY: 22 * (f - 1) }, { scaleY: f }, { scaleX: 1 - 0.08 * Math.sin(c * 0.11) }] };
  });
  return (
    <Animated.View style={[{ position: 'absolute', left: 14, top: 104, width: 32, height: 60 }, a]}>
      <Svg width={32} height={60} viewBox="0 0 32 60">
        <Path d="M3,0 L29,0 C30,22 22,40 16,60 C10,40 2,22 3,0 Z" fill="#FFB547" />
        <Path d="M9,0 L23,0 C24,14 20,26 16,38 C12,26 8,14 9,0 Z" fill="#FFF1B8" />
      </Svg>
    </Animated.View>
  );
}

export const liftOff: SceneFactory = (m, t) => {
  const counts = [200, 650, 1100];
  const lift = 1450;
  const fly = 1150;
  const slam = 2550;
  const startY = (h: number) => h + 150;
  const riseY = (h: number, ms: number) => {
    const y0 = startY(h);
    const acc = (2 * (y0 + 260)) / Math.pow(fly / 1000, 2);
    const s = Math.max(0, ms - lift) / 1000;
    return y0 - 0.5 * acc * s * s;
  };
  return {
    duration: 4200,
    bg: '#0B1026',
    dim: 0.97,
    impacts: [
      ...counts.map((c) => ({ at: c, amp: 4 })),
      { at: lift, amp: 6 },
      { at: lift + 160, amp: 4 },
      { at: lift + 320, amp: 3 },
      { at: slam, amp: 7 },
    ],
    Render: function Lift({ clock, w, h, rand, cx, cy }: SceneProps) {
      const stars = useMemo(
        () => Array.from({ length: 70 }, () => ({ x: rand.range(0, w), y: rand.range(0, h), r: rand.range(0.6, 2), o: rand.range(0.3, 0.9) })),
        [rand, w, h],
      );
      const list = useMemo<Particle[]>(() => {
        const tail = (ms: number) => riseY(h, ms) + 50;
        const smoke = Array.from({ length: 16 }, (_, i) => {
          const at = lift + 120 + i * 50;
          return {
            x: cx + rand.range(-8, 8),
            y: tail(at) + 30,
            vx: rand.range(-70, 70),
            vy: rand.range(20, 90),
            g: 0,
            k: 1.5,
            at,
            life: 1500,
            fadeOut: 1200,
            grow: [0.6, 3] as [number, number],
            w: 34,
            h: 34,
            color: 'rgba(201,206,223,0.3)',
            shape: 'circle',
          } as Particle;
        });
        const fire = Array.from({ length: 22 }, (_, i) => {
          const at = lift + i * 40;
          return {
            x: cx + rand.range(-6, 6),
            y: Math.min(tail(at) + 50, h + 20),
            vx: rand.range(-60, 60),
            vy: rand.range(250, 450),
            g: 0,
            k: 2,
            at,
            life: 420,
            fadeOut: 300,
            w: 7,
            h: 7,
            color: rand.pick(['#FFB547', '#FF7A45', '#FFF1B8']),
            shape: 'circle',
          } as Particle;
        });
        const cloud = [-1, 1].flatMap((s) =>
          burst(rand, {
            x: cx,
            y: h + 10,
            at: lift,
            count: 9,
            dir: s === 1 ? -12 : -168,
            spread: 34,
            speed: [220, 520],
            g: -40,
            k: 2.2,
            life: [1600, 2100],
            fadeOut: 1100,
            grow: [1, 3],
            look: () => ({ shape: 'circle', color: 'rgba(185,190,208,0.55)', w: 46, h: 46 }),
          }),
        );
        const party_ = burst(rand, {
          x: cx,
          y: cy,
          at: slam,
          count: 30,
          speed: [400, 900],
          g: 900,
          k: 1.8,
          life: [1200, 1700],
          look: paper(party(t)),
        });
        return [...smoke, ...cloud, ...fire, ...party_];
      }, [rand, cx, cy, h]);
      const path = useMemo(() => {
        const ts = Array.from({ length: 9 }, (_, i) => lift + (fly * i) / 8);
        return { t: [0, ...ts], y: [startY(h) - 65, ...ts.map((ms) => riseY(h, ms) - 65)] };
      }, [h]);
      const size = numberSize(m.n);
      return (
        <>
          <Svg pointerEvents="none" width={w} height={h} style={StyleSheet.absoluteFill}>
            {stars.map((s, i) => (
              <Circle key={i} cx={s.x} cy={s.y} r={s.r} fill="#FFF6D8" fillOpacity={s.o} />
            ))}
          </Svg>
          {counts.map((c, i) => (
            <Keyed
              key={i}
              clock={clock}
              style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0 }}
              k={{ t: [c + 300, c + 420], o: [1, 0], s: [1, 1.5] }}>
              <View style={{ position: 'absolute', top: cy - 110, left: 0, right: 0, alignItems: 'center' }}>
                <Slam clock={clock} at={c} from={2.4} fall={150}>
                  <BigNumber value={3 - i} size={200} color="#FFFFFF" font={t.font.display} />
                </Slam>
              </View>
            </Keyed>
          ))}
          {counts.map((c, i) => (
            <Shockwave key={`s${i}`} clock={clock} at={c} x={cx} y={cy} color="#FFFFFF" size={320} width={4} dur={450} />
          ))}
          <Particles clock={clock} list={list} />
          <Keyed clock={clock} style={{ position: 'absolute', left: cx - 30, top: 0, width: 60, height: 130 }} k={path}>
            <Flame clock={clock} />
            <Rocket />
          </Keyed>
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#FFE9A8" opacity={0.18} />
          <Flash clock={clock} at={slam} color="#FFFFFF" peak={0.3} />
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 250} cy={cy} size={size} color="#FFFFFF" glow="#FFE9A8" ink="#FFFFFF" sub="#C9D2EA" />
        </>
      );
    },
  };
};
