import { useMemo } from 'react';
import Svg, { Circle, Defs, G, LinearGradient, Path, Polygon, Rect, Stop } from 'react-native-svg';

import { burst, cannons, paper, type Particle, Particles, rain, sparks } from '@/components/celebration/engine';
import { Flash, Glow, Rays, Shockwave } from '@/components/celebration/effects';
import { Keyed } from '@/components/celebration/motion';
import { NAVY, TRICOLORE } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function Sword({ len }: { len: number }) {
  const w = len * 0.2;
  return (
    <Svg width={w} height={len} viewBox="0 0 44 220">
      <Defs>
        <LinearGradient id="blade" x1="0" y1="0" x2="1" y2="0">
          <Stop offset="0" stopColor="#F4F7FF" />
          <Stop offset="0.5" stopColor="#B8C2D8" />
          <Stop offset="1" stopColor="#7E8AA6" />
        </LinearGradient>
      </Defs>
      <Polygon points="22,0 28,14 28,150 16,150 16,14" fill="url(#blade)" />
      <Path d="M22 8 L22 146" stroke="#8D98B4" strokeWidth={1.2} />
      <Rect x={2} y={148} width={40} height={9} rx={4} fill="#FFC21A" stroke="#B37400" strokeWidth={1.5} />
      <Rect x={17.5} y={157} width={9} height={44} rx={3} fill="#6B3A1E" />
      <G stroke="#3E2010" strokeWidth={1}>
        <Path d="M17.5 166 L26.5 170 M17.5 176 L26.5 180 M17.5 186 L26.5 190" />
      </G>
      <Circle cx={22} cy={208} r={8} fill="#FFC21A" stroke="#B37400" strokeWidth={1.5} />
    </Svg>
  );
}

export const marignan: SceneFactory = (m, t) => {
  const clash = 1000;
  const slam = 1500;
  return {
    duration: 5800,
    bg: NAVY,
    dim: 0.97,
    impacts: [
      { at: clash, amp: 10 },
      { at: slam, amp: 5 },
    ],
    Render: function Marignan({ clock, w, h, rand, cx, cy }: SceneProps) {
      const len = 210;
      const sy = cy - 190;
      const list = useMemo<Particle[]>(
        () => [
          ...burst(rand, {
            x: cx,
            y: sy,
            at: clash,
            count: 30,
            speed: [450, 1100],
            g: 700,
            k: 2.2,
            life: [500, 900],
            look: sparks(['#FFFFFF', '#FFE082', '#FFC21A'], 3),
          }),
          ...burst(rand, {
            x: cx,
            y: sy,
            at: clash + 30,
            count: 12,
            speed: [100, 300],
            g: 200,
            k: 2,
            life: [800, 1200],
            look: (r) => ({ shape: 'circle', color: r.pick(['#FFFFFF', '#FFE082']), w: 4, h: 4 }),
          }),
          ...cannons(rand, {
            w,
            y: h + 10,
            at: slam,
            delay: 60,
            count: 30,
            dir: -62,
            spread: 26,
            speed: [1100, 1700],
            g: 950,
            k: 1.3,
            life: [2200, 2800],
            look: paper(TRICOLORE),
          }),
          ...rain(rand, { w, h, count: 26, from: slam + 600, to: 3800, look: paper(TRICOLORE), life: 2600 }),
        ],
        [rand, w, h, cx, sy],
      );
      const times = [0, clash - 380, clash, clash + 90, clash + 300, clash + 520];
      const size = numberSize(m.n);
      const sword = (side: -1 | 1) => (
        <Keyed
          clock={clock}
          k={{
            t: times,
            x: [side * w * 0.75, side * w * 0.75, 0, side * 14, side * -3, 0],
            y: [80, 80, 0, -6, 2, 0],
            r: [side * 110, side * 110, side * 38, side * 50, side * 36, side * 40],
            o: [0, 0, 1, 1, 1, 1],
          }}
          style={{ position: 'absolute', left: cx - len * 0.1, top: sy - len * 0.5 }}>
          <Sword len={len} />
        </Keyed>
      );
      return (
        <>
          <Glow clock={clock} at={clash} x={cx} y={sy} color="#FFE082" size={260} opacity={0.6} />
          <Rays clock={clock} at={clash} x={cx} y={sy} color="#FFFFFF" size={520} count={12} opacity={0.14} speed={10} />
          <Flash clock={clock} at={clash} color="#FFFFFF" peak={0.55} fall={300} />
          <Shockwave clock={clock} at={clash} x={cx} y={sy} color="#FFFFFF" size={420} width={6} dur={550} />
          {sword(-1)}
          {sword(1)}
          <Particles clock={clock} list={list} />
          <SlamNumber
            clock={clock}
            m={m}
            t={t}
            slam={slam}
            at={slam + 300}
            cy={cy + 40}
            size={size}
            color="#FFFFFF"
            glow="#2F6BFF"
            ink="#FFFFFF"
            sub="#B9C2E0"
            from={2.6}
          />
        </>
      );
    },
  };
};
