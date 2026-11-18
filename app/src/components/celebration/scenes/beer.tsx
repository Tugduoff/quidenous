/** 1664: beer — two mugs clink, the screen fills up with foam on top. */
import { useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, G, LinearGradient, Path, Rect, Stop } from 'react-native-svg';

import { burst, type Clock, type Particle, Particles, sparks } from '@/components/celebration/engine';
import { Flash, Rays, Shockwave } from '@/components/celebration/effects';
import { Keyed } from '@/components/celebration/motion';
import { isDark } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function Mug({ glass, flip }: { glass: string; flip?: boolean }) {
  return (
    <Svg width={92} height={112} viewBox="0 0 92 112">
      <G transform={flip ? 'translate(92,0) scale(-1,1)' : undefined}>
        <Path d="M66,38 C86,38 88,46 88,62 C88,80 84,88 66,88" stroke={glass} strokeWidth={8} fill="none" />
        <Rect x={8} y={22} width={60} height={86} rx={10} fill="#F2A900" />
        <Rect x={14} y={30} width={10} height={70} rx={5} fill="#FFFFFF" fillOpacity={0.25} />
        <Rect x={8} y={22} width={60} height={86} rx={10} fill="none" stroke={glass} strokeWidth={4} />
        {[14, 26, 38, 50, 62].map((x, i) => (
          <Circle key={x} cx={x} cy={i % 2 ? 16 : 20} r={i % 2 ? 12 : 10} fill="#FFFDF4" />
        ))}
      </G>
    </Svg>
  );
}

function Bob({ clock, i, x, r }: { clock: Clock; i: number; x: number; r: number }) {
  const a = useAnimatedStyle(() => ({ transform: [{ translateY: Math.sin(clock.value * 0.005 + i * 1.3) * 4 }] }));
  return (
    <Animated.View
      style={[{ position: 'absolute', left: x - r, top: -r * 0.9, width: r * 2, height: r * 2, borderRadius: r, backgroundColor: '#FFFDF4' }, a]}
    />
  );
}

export const beer: SceneFactory = (m, t) => {
  const clink = 1150;
  const slam = 1500;
  return {
    duration: 4000,
    bg: t.bg,
    impacts: [
      { at: clink, amp: 5 },
      { at: slam, amp: 7 },
    ],
    Render: function Beer({ clock, w, h, rand, cx, cy }: SceneProps) {
      const dark = isDark(t);
      const top = Math.min(h - 120, Math.max(h * 0.66, cy + 240));
      const body = h - top;
      const my = Math.max(70, cy - 215);
      const foam = useMemo(() => {
        const out: { x: number; r: number }[] = [];
        for (let x = -10; x < w + 20; x += rand.range(20, 30)) out.push({ x, r: rand.range(13, 22) });
        return out;
      }, [rand, w]);
      const bubbles = useMemo<Particle[]>(
        () =>
          Array.from({ length: 34 }, () => {
            const s = rand.range(4, 10);
            return {
              x: rand.range(0, w),
              y: body + 10,
              vx: 0,
              vy: -rand.range(60, 160),
              g: -90,
              k: 0.3,
              sway: rand.range(3, 9),
              swayHz: rand.range(0.8, 2),
              at: rand.range(0, 3600),
              life: 3000,
              w: s,
              h: s,
              color: '#FFF6D8',
              shape: 'circle',
              fadeIn: 200,
            } as Particle;
          }),
        [rand, w, body],
      );
      const fx = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: my - 30,
            at: clink,
            count: 18,
            dir: -90,
            spread: 140,
            speed: [250, 600],
            g: 1200,
            k: 1,
            life: [700, 1100],
            look: (r) => {
              const s = r.range(5, 11);
              return { shape: 'circle', color: r.pick(['#F2A900', '#FFD54A', '#FFFDF4']), w: s, h: s };
            },
          }),
          ...burst(rand, {
            x: cx,
            y: my - 30,
            at: clink,
            count: 10,
            speed: [200, 420],
            g: 200,
            k: 2.5,
            life: [350, 600],
            look: sparks(['#FFFFFF', '#FFF1B8'], 2),
          }),
        ],
        [rand, cx, my],
      );
      const glass = dark ? '#FFFFFFCC' : '#9C9C94';
      const size = numberSize(m.n);
      const ink = dark ? '#F2A900' : '#B26A00';
      return (
        <>
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: -20, top, width: w + 40, height: body + 40 }}
            k={{ t: [100, 700, 1300, 1700], y: [body + 40, body * 0.45, body * 0.06, 0] }}>
            <View style={{ position: 'absolute', left: 0, top: 0, width: w + 40, height: body + 40, overflow: 'hidden' }}>
              <Svg width={w + 40} height={body + 40} style={StyleSheet.absoluteFill}>
                <Defs>
                  <LinearGradient id="beer" x1="0" y1="0" x2="0" y2="1">
                    <Stop offset="0" stopColor="#FFC531" />
                    <Stop offset="1" stopColor="#C77700" />
                  </LinearGradient>
                </Defs>
                <Rect x={0} y={0} width={w + 40} height={body + 40} fill="url(#beer)" />
              </Svg>
              <Particles clock={clock} list={bubbles} />
            </View>
            {foam.map((f, i) => (
              <Bob key={i} clock={clock} i={i} x={f.x} r={f.r} />
            ))}
          </Keyed>
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#FFC531" opacity={0.2} />
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: 0, top: my - 56, width: 92, height: 112 }}
            k={{ t: [150, clink, clink + 80, clink + 260], x: [-110, cx - 82, cx - 92, cx - 88], r: [0, 16, 8, 11] }}>
            <Mug glass={glass} flip />
          </Keyed>
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: 0, top: my - 56, width: 92, height: 112 }}
            k={{ t: [150, clink, clink + 80, clink + 260], x: [w + 20, cx - 10, cx, cx - 4], r: [0, -16, -8, -11] }}>
            <Mug glass={glass} />
          </Keyed>
          <Shockwave clock={clock} at={clink} x={cx} y={my - 20} color="#FFD54A" size={220} width={4} dur={500} />
          <Particles clock={clock} list={fx} />
          <Flash clock={clock} at={slam} color="#FFC531" peak={0.25} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FFC531" size={480} width={7} />
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 250} cy={cy} size={size} color={ink} />
        </>
      );
    },
  };
};
