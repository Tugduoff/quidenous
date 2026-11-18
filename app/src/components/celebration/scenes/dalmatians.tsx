/** 101: dalmatians — spots pop up all over, paw prints cross the screen. */
import { useMemo } from 'react';
import Svg, { Ellipse, G } from 'react-native-svg';

import { burst, Particles } from '@/components/celebration/engine';
import { Shockwave } from '@/components/celebration/effects';
import { centred } from '@/components/celebration/layout';
import { Keyed } from '@/components/celebration/motion';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function Spot({ size, rot, a, b }: { size: number; rot: number; a: number; b: number }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 100 100">
      <G transform={`rotate(${rot} 50 50)`}>
        <Ellipse cx={50} cy={52} rx={34} ry={27} fill="#141414" />
        <Ellipse cx={50 + a * 70} cy={50 + b * 50} rx={19} ry={16} fill="#141414" />
        <Ellipse cx={50 - b * 50} cy={52 + a * 40} rx={14} ry={12} fill="#141414" />
      </G>
    </Svg>
  );
}

function Paw({ size, color, rot }: { size: number; color: string; rot: number }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 40 40">
      <G transform={`rotate(${rot} 20 20)`} fill={color}>
        <Ellipse cx={20} cy={26} rx={9.5} ry={8} />
        <Ellipse cx={8.5} cy={15} rx={3.8} ry={4.8} />
        <Ellipse cx={15.5} cy={8.5} rx={3.8} ry={4.8} />
        <Ellipse cx={24.5} cy={8.5} rx={3.8} ry={4.8} />
        <Ellipse cx={31.5} cy={15} rx={3.8} ry={4.8} />
      </G>
    </Svg>
  );
}

export const dalmatians: SceneFactory = (m, t) => {
  const slam = 1500;
  return {
    duration: 4000,
    bg: '#F7F4EE',
    dim: 0.98,
    impacts: [{ at: slam, amp: 7 }],
    Render: function Dalmatians({ clock, w, h, rand, cx, cy }: SceneProps) {
      const spots = useMemo(() => {
        const out: { x: number; y: number; s: number; rot: number; at: number; a: number; b: number }[] = [];
        for (let tries = 0; out.length < 22 && tries < 400; tries++) {
          const x = rand.range(0, w);
          const y = rand.range(10, h - 10);
          // Keep the number and its text readable.
          if (Math.abs(y - (cy + 60)) < 175 && Math.abs(x - cx) < 175) continue;
          if (out.some((o) => Math.hypot(o.x - x, o.y - y) < 70)) continue;
          out.push({
            x,
            y,
            s: rand.range(34, 74),
            rot: rand.range(0, 180),
            at: rand.range(0, 1300),
            a: rand.range(-0.5, 0.5),
            b: rand.range(-0.5, 0.5),
          });
        }
        return out;
      }, [rand, w, h, cx, cy]);
      const paws = useMemo(() => {
        const lane = (y0: number, y1: number, from: number, dir: 1 | -1, n: number) =>
          Array.from({ length: n }, (_, i) => {
            const k = i / (n - 1);
            const x = dir === 1 ? -10 + k * (w + 20) : w + 10 - k * (w + 20);
            return { x, y: y0 + (y1 - y0) * k + (i % 2 ? 11 : -11), at: from + i * 170, rot: dir === 1 ? 90 : -90 };
          });
        return [...lane(h * 0.86, h * 0.8, 300, 1, 9), ...lane(h * 0.1, h * 0.14, 1700, -1, 9)];
      }, [w, h]);
      const pop = useMemo(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 22,
            speed: [300, 750],
            g: 900,
            k: 2,
            life: [900, 1400],
            look: (r) => {
              const s = r.range(6, 13);
              return { shape: 'circle', color: '#141414', w: s, h: s };
            },
          }),
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          {spots.map((s, i) => (
            <Keyed key={i} clock={clock} style={centred(s.x, s.y, s.s, s.s)} k={{ t: [s.at, s.at + 120, s.at + 220], s: [0, 1.15, 1], o: [0, 1, 1] }}>
              <Spot size={s.s} rot={s.rot} a={s.a} b={s.b} />
            </Keyed>
          ))}
          {paws.map((p, i) => (
            <Keyed
              key={`p${i}`}
              clock={clock}
              style={centred(p.x, p.y, 26, 26)}
              k={{ t: [p.at, p.at + 90, p.at + 1100, p.at + 1500], s: [0.4, 1, 1, 1], o: [0, 0.85, 0.85, 0] }}>
              <Paw size={26} color="#3A3A3A" rot={p.rot} />
            </Keyed>
          ))}
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#141414" size={420} width={5} />
          <Particles clock={clock} list={pop} />
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 250} cy={cy} size={size} color="#121216" ink="#121216" sub="#55555E" />
        </>
      );
    },
  };
};
