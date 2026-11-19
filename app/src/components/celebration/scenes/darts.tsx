import { useMemo } from 'react';
import Svg, { Circle, G, Path, Rect } from 'react-native-svg';

import { sectorPath } from '@/lib/utils/svg-paths';

import { burst, paper, Particles, sparks } from '@/components/celebration/engine';
import { Flash, Rays, Shockwave } from '@/components/celebration/effects';
import { centred } from '@/components/celebration/layout';
import { Keyed } from '@/components/celebration/motion';
import { party } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function Dartboard({ size }: { size: number }) {
  const parts = useMemo(
    () =>
      Array.from({ length: 20 }, (_, i) => {
        const a = -90 + i * 18;
        const even = i % 2 === 0;
        return {
          single: sectorPath(0, 80, a - 9, a + 9),
          double: sectorPath(80, 87, a - 9, a + 9),
          triple: sectorPath(50, 57, a - 9, a + 9),
          base: even ? '#1B1B1B' : '#F1E6C8',
          ring: even ? '#D7263D' : '#1F8A4C',
        };
      }),
    [],
  );
  return (
    <Svg width={size} height={size} viewBox="-100 -100 200 200">
      <Circle cx={0} cy={0} r={99} fill="#151515" />
      <Circle cx={0} cy={0} r={99} fill="none" stroke="#2C2C2C" strokeWidth={2} />
      {parts.map((p, i) => (
        <G key={i}>
          <Path d={p.single} fill={p.base} />
          <Path d={p.double} fill={p.ring} />
          <Path d={p.triple} fill={p.ring} />
        </G>
      ))}
      <Circle cx={0} cy={0} r={9} fill="#1F8A4C" />
      <Circle cx={0} cy={0} r={4.5} fill="#D7263D" />
    </Svg>
  );
}

function Dart({ flight }: { flight: string }) {
  return (
    <Svg width={96} height={26} viewBox="0 0 96 26">
      <Path d="M0,1 L24,13 L0,25 L9,13 Z" fill={flight} />
      <Path d="M2,4 L22,13 L2,22" stroke="#00000033" strokeWidth={1.2} fill="none" />
      <Rect x={14} y={11.5} width={26} height={3} fill="#3A3A3A" />
      <Rect x={38} y={8.5} width={30} height={9} rx={3} fill="#A0A6AD" />
      {[44, 49, 54, 59].map((x) => (
        <Rect key={x} x={x} y={8.5} width={1.6} height={9} fill="#6B7178" />
      ))}
      <Path d="M67,11.2 L96,13 L67,14.8 Z" fill="#E3E3E3" />
    </Svg>
  );
}

export const darts: SceneFactory = (m, t) => {
  const hits = [520, 900, 1280];
  const slam = 1700;
  return {
    duration: 4100,
    bg: t.bg,
    impacts: [...hits.map((a) => ({ at: a, amp: 4 })), { at: slam, amp: 8 }],
    Render: function Darts({ clock, w, h, rand, cx, cy }: SceneProps) {
      const board = Math.min(210, cy * 0.9);
      const by = Math.max(board / 2 + 24, cy - 150 - board / 2 + 30);
      const r = (53.5 / 200) * board;
      const L = 96;
      const flights = useMemo(() => {
        return hits.map((T, i) => {
          const px = cx + (i - 1) * (board * 0.045);
          const py = by - r;
          const sx = cx + 70 + i * 50;
          const sy = h + 60;
          const th = Math.atan2(py - sy, px - sx);
          const end = 0.72;
          const ex = px - (L / 2) * end * Math.cos(th);
          const ey = py - (L / 2) * end * Math.sin(th);
          const deg = (th * 180) / Math.PI;
          return {
            px,
            py,
            k: {
              t: [T - 281, T - 280, T, T + 60, T + 140, T + 240],
              x: [sx - L / 2, sx - L / 2, ex - L / 2, ex - L / 2, ex - L / 2, ex - L / 2],
              y: [sy - 13, sy - 13, ey - 13, ey - 13, ey - 13, ey - 13],
              r: [deg, deg, deg, deg + 6, deg - 3, deg],
              s: [1.8, 1.8, end, end, end, end],
              o: [0, 1, 1, 1, 1, 1],
            },
          };
        });
      }, [cx, by, r, h, board]);
      const wobble = useMemo(() => {
        const tt = [0, 200, 330];
        const rr = [0, 0, 0];
        const ss = [0.3, 1.08, 1];
        for (const T of hits) {
          tt.push(T, T + 70, T + 160, T + 280);
          rr.push(0, 2.5, -1.2, 0);
          ss.push(1, 1.03, 1, 1);
        }
        return { t: tt, r: rr, s: ss, o: [0, 1, 1, ...hits.flatMap(() => [1, 1, 1, 1])] };
      }, []);
      const fx = useMemo(
        () => [
          ...flights.flatMap((f, i) =>
            burst(rand, {
              x: f.px,
              y: f.py,
              at: hits[i],
              count: 9,
              speed: [160, 420],
              g: 700,
              k: 2.5,
              life: [300, 600],
              look: sparks(['#FFD54A', '#FFF1B8', t.acc], 2),
            }),
          ),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 34, speed: [400, 900], g: 900, k: 1.8, life: [1300, 1900], look: paper(party(t)) }),
        ],
        [flights, rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Rays clock={clock} at={slam} x={cx} y={cy} color={t.acc} opacity={0.16} />
          <Keyed clock={clock} style={centred(cx, by, board, board)} k={wobble}>
            <Dartboard size={board} />
          </Keyed>
          {flights.map((f, i) => (
            <Shockwave key={i} clock={clock} at={hits[i]} x={f.px} y={f.py} color="#FFD54A" size={90} width={3} dur={400} />
          ))}
          {flights.map((f, i) => (
            <Keyed key={`d${i}`} clock={clock} style={{ position: 'absolute', left: 0, top: 0, width: L, height: 26 }} k={f.k}>
              <Dart flight={i === 1 ? '#FF4FA3' : t.acc} />
            </Keyed>
          ))}
          <Flash clock={clock} at={slam} color={t.acc} peak={0.3} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color={t.acc} size={480} width={7} />
          <Particles clock={clock} list={fx} />
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 250} cy={cy} size={size} color={t.accInk} />
        </>
      );
    },
  };
};
