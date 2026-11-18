/** 300: bowling — a strike, the pins fly. */
import { useMemo } from 'react';
import { StyleSheet } from 'react-native';
import Svg, { Circle, Defs, LinearGradient, Path, RadialGradient, Rect, Stop } from 'react-native-svg';

import { burst, paper, Particles, sparks } from '@/components/celebration/engine';
import { Flash, Rays, Shockwave } from '@/components/celebration/effects';
import { Keyed, type Throw, Thrown } from '@/components/celebration/motion';
import { isDark, party } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function Pin({ w }: { w: number }) {
  return (
    <Svg width={w} height={w * (60 / 22)} viewBox="0 0 22 60">
      <Path
        d="M11 0 C16 0 17 6 16 11 C15 15 13.2 17 13.2 20 C13.2 24 21 32 21 44 C21 54 17 60 11 60 C5 60 1 54 1 44 C1 32 8.8 24 8.8 20 C8.8 17 7 15 6 11 C5 6 6 0 11 0 Z"
        fill="#FFFFFF"
        stroke="#00000030"
        strokeWidth={1}
      />
      <Rect x={7.4} y={16.5} width={7.2} height={1.8} fill="#D7263D" />
      <Rect x={7.2} y={20} width={7.6} height={1.8} fill="#D7263D" />
      <Path d="M8 4 C7.5 6 7.5 9 8.5 11" stroke="#00000014" strokeWidth={1.5} fill="none" />
    </Svg>
  );
}

function Ball({ size, color }: { size: number; color: string }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 60 60">
      <Defs>
        <RadialGradient id="ballShine" cx="35%" cy="30%" r="70%">
          <Stop offset="0" stopColor="#FFFFFF" stopOpacity={0.45} />
          <Stop offset="0.5" stopColor="#FFFFFF" stopOpacity={0} />
        </RadialGradient>
      </Defs>
      <Circle cx={30} cy={30} r={29} fill={color} />
      <Circle cx={30} cy={30} r={29} fill="url(#ballShine)" />
      <Circle cx={24} cy={20} r={4} fill="#00000099" />
      <Circle cx={34} cy={19} r={4} fill="#00000099" />
      <Circle cx={30} cy={31} r={5} fill="#00000099" />
    </Svg>
  );
}

export const bowling: SceneFactory = (m, t) => {
  const hit = 850;
  const slam = 1400;
  return {
    duration: 4000,
    bg: t.bg,
    impacts: [
      { at: hit, amp: 7 },
      { at: slam, amp: 6 },
    ],
    Render: function Bowling({ clock, w, h, rand, cx, cy }: SceneProps) {
      const py = Math.max(150, cy - 175);
      const pins = useMemo(() => {
        const out: { p: Throw; w: number }[] = [];
        // Back row first: front pins are drawn over them.
        for (let r = 3; r >= 0; r--) {
          const sc = 1 - r * 0.1;
          for (let j = 0; j <= r; j++) {
            const pw = 24 * sc;
            const ph = pw * (60 / 22);
            const x = cx + (j - r / 2) * 38 * sc;
            const y = py - r * 17 - ph / 2;
            out.push({
              w: pw,
              p: {
                x,
                y,
                vx: (j - r / 2) * rand.range(160, 340) + rand.range(-140, 140),
                vy: -rand.range(520, 980),
                g: 2300,
                k: 0.3,
                spin: rand.sign() * rand.range(300, 900),
                rot: 0,
                at: hit + (3 - r) * 0 + r * 35,
                w: pw,
                h: ph,
              },
            });
          }
        }
        return out;
      }, [rand, cx, py]);
      const lane = useMemo(() => {
        const top = py - 70;
        const tw = 90;
        const bw = Math.min(w * 0.9, 340);
        return `M${cx - tw},${top} L${cx + tw},${top} L${cx + bw},${h} L${cx - bw},${h} Z`;
      }, [cx, py, w, h]);
      const fx = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: py - 20,
            at: hit,
            count: 14,
            speed: [250, 600],
            g: 900,
            k: 2,
            life: [500, 900],
            look: sparks(['#FFFFFF', '#FFD54A', t.acc], 3),
          }),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 34, speed: [400, 900], g: 900, k: 1.8, life: [1300, 1900], look: paper(party(t)) }),
        ],
        [rand, cx, cy, py],
      );
      const ballSize = 64;
      const size = numberSize(m.n);
      const dark = isDark(t);
      return (
        <>
          <Svg pointerEvents="none" width={w} height={h} style={StyleSheet.absoluteFill}>
            <Defs>
              <LinearGradient id="lane" x1="0" y1="0" x2="0" y2="1">
                <Stop offset="0" stopColor={dark ? '#FFFFFF' : '#000000'} stopOpacity={0.02} />
                <Stop offset="1" stopColor={dark ? '#FFFFFF' : '#000000'} stopOpacity={0.07} />
              </LinearGradient>
            </Defs>
            <Path d={lane} fill="url(#lane)" stroke={t.line} strokeWidth={2} />
          </Svg>
          <Rays clock={clock} at={slam} x={cx} y={cy} color={t.acc} opacity={0.16} />
          <Shockwave clock={clock} at={hit} x={cx} y={py - 30} color={t.acc} size={300} width={5} />
          {pins.map((p, i) => (
            <Thrown key={i} clock={clock} p={p.p}>
              <Pin w={p.w} />
            </Thrown>
          ))}
          <Keyed
            clock={clock}
            style={{ position: 'absolute', left: cx - ballSize / 2, top: 0, width: ballSize, height: ballSize }}
            k={{
              t: [0, hit, hit + 300],
              y: [h + 40, py - 30, py - 140],
              x: [30, 0, -6],
              r: [0, -900, -1100],
              s: [1.6, 0.62, 0.4],
              o: [1, 1, 0],
            }}>
            <Ball size={ballSize} color={dark ? '#5B47FF' : '#3D2BD1'} />
          </Keyed>
          <Flash clock={clock} at={slam} color={t.acc} peak={0.3} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color={t.acc} size={480} width={7} />
          <Particles clock={clock} list={fx} />
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 250} cy={cy} size={size} color={t.accInk} />
        </>
      );
    },
  };
};
