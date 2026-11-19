import { useMemo } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';

import { clamp01 } from '@/lib/utils/easing';
import { mergeCellTiles, mergeChain, twinTiles } from '@/lib/utils/merge-chain';

import { burst, cannons, type Clock, paper, Particles } from '@/components/celebration/engine';
import { Flash, Glow, Rays, Shockwave } from '@/components/celebration/effects';
import { ABS } from '@/components/celebration/layout';
import { FadeAway, Pop, Slam, Window } from '@/components/celebration/motion';
import { GOLD } from '@/components/celebration/palettes';
import { Headline, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

const TILE: Record<number, [string, string]> = {
  2: ['#EEE4DA', '#776E65'],
  4: ['#EDE0C8', '#776E65'],
  8: ['#F2B179', '#FFFFFF'],
  16: ['#F59563', '#FFFFFF'],
  32: ['#F67C5F', '#FFFFFF'],
  64: ['#F65E3B', '#FFFFFF'],
  128: ['#EDCF72', '#FFFFFF'],
  256: ['#EDCC61', '#FFFFFF'],
  512: ['#EDC850', '#FFFFFF'],
  1024: ['#EDC53F', '#FFFFFF'],
  2048: ['#EDC22E', '#FFFFFF'],
};

function Tile({ v, size, font }: { v: number; size: number; font: string }) {
  const [bg, ink] = TILE[v];
  const digits = String(v).length;
  const fs = size * (digits <= 2 ? 0.5 : digits === 3 ? 0.4 : 0.31);
  return (
    <View style={{ width: size, height: size, borderRadius: size * 0.12, backgroundColor: bg, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ fontFamily: font, fontSize: fs, color: ink, letterSpacing: -1, includeFontPadding: false }}>{v}</Text>
    </View>
  );
}

export const tiles: SceneFactory = (m, t) => {
  const chain = mergeChain();
  const { merges, last, slam } = chain;
  return {
    duration: 6200,
    bg: t.bg,
    impacts: [...merges.slice(5).map((a) => ({ at: a, amp: 1.5 })), { at: last, amp: 4 }, { at: slam, amp: 9 }],
    Render: function Tiles({ clock, w, h, rand, cx, cy }: SceneProps) {
      const c = 64;
      const gap = 10;
      const B = c * 4 + gap * 5;
      const bx = cx - B / 2;
      const by = cy - B / 2;
      const cell = (r: number, col: number) => ({ x: bx + gap + col * (c + gap), y: by + gap + r * (c + gap) });
      const M = cell(1, 1);
      const sources = [cell(1, 3), cell(3, 1), cell(1, 0), cell(0, 1)];
      const font = t.font.display;
      const list = useMemo(
        () => [
          ...burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 44,
            speed: [450, 1000],
            g: 900,
            k: 1.7,
            life: [1500, 2200],
            look: paper([...GOLD, '#F67C5F', '#F2B179']),
          }),
          ...cannons(rand, {
            w,
            y: h * 0.75,
            at: slam + 80,
            count: 16,
            dir: -58,
            spread: 28,
            speed: [900, 1400],
            g: 900,
            k: 1.3,
            life: [1800, 2300],
            look: paper(['#EDC22E', '#F59563', '#EEE4DA']),
          }),
        ],
        [rand, w, h, cx, cy],
      );
      return (
        <>
          <FadeAway clock={clock} from={last + 80} to={slam - 40} style={StyleSheet.absoluteFill}>
            <Pop clock={clock} at={0} style={{ position: 'absolute', left: bx, top: by }}>
              <View style={{ width: B, height: B, borderRadius: 12, backgroundColor: '#BBADA0' }}>
                {Array.from({ length: 16 }, (_, i) => {
                  const p = cell(Math.floor(i / 4), i % 4);
                  return (
                    <View
                      key={i}
                      style={{
                        position: 'absolute',
                        left: p.x - bx,
                        top: p.y - by,
                        width: c,
                        height: c,
                        borderRadius: 8,
                        backgroundColor: '#CDC1B4',
                      }}
                    />
                  );
                })}
                {(
                  [
                    [0, 3, 4],
                    [3, 3, 2],
                    [2, 2, 4],
                    [3, 0, 2],
                  ] as const
                ).map(([r, col, v]) => {
                  const p = cell(r, col);
                  return (
                    <View key={`${r}${col}`} style={{ position: 'absolute', left: p.x - bx, top: p.y - by }}>
                      <Tile v={v} size={c} font={font} />
                    </View>
                  );
                })}
              </View>
            </Pop>
            {mergeCellTiles(chain).map(({ v, from, to }) => (
              <Window key={v} clock={clock} from={from} to={to} style={{ position: 'absolute', left: M.x, top: M.y }}>
                <Pop clock={clock} at={from}>
                  <Tile v={v} size={c} font={font} />
                </Pop>
              </Window>
            ))}
            {twinTiles(chain).map(({ v, spawn, start, end, source }) => (
              <Twin key={v} clock={clock} v={v} size={c} font={font} from={sources[source]} to={M} spawn={spawn} start={start} end={end} />
            ))}
          </FadeAway>
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#EDC22E" opacity={0.3} />
          <Glow clock={clock} at={slam} x={cx} y={cy} color="#EDC22E" size={380} opacity={0.4} pulse={0.7} />
          <Flash clock={clock} at={slam} color="#FFE082" peak={0.4} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#EDC22E" size={560} width={8} dur={700} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 280} cy={cy} box={196}>
            <Slam clock={clock} at={slam} from={2.6}>
              <Tile v={2048} size={180} font={font} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};

function Twin({
  clock,
  v,
  size,
  font,
  from,
  to,
  spawn,
  start,
  end,
}: {
  clock: Clock;
  v: number;
  size: number;
  font: string;
  from: { x: number; y: number };
  to: { x: number; y: number };
  spawn: number;
  start: number;
  end: number;
}) {
  const [fx, fy, tx, ty] = [from.x, from.y, to.x, to.y];
  const anim = useAnimatedStyle(() => {
    const c = clock.value;
    if (c < spawn || c >= end) return { opacity: 0, transform: [{ translateX: -9999 }, { translateY: 0 }, { scale: 1 }] };
    const p = clamp01((c - start) / (end - start));
    const e = p * p;
    const s = interpolate(c, [spawn, spawn + 120, spawn + 170], [0.2, 1.08, 1], Extrapolation.CLAMP);
    return { opacity: 1, transform: [{ translateX: fx + (tx - fx) * e }, { translateY: fy + (ty - fy) * e }, { scale: s }] };
  });
  return (
    <Animated.View style={[ABS, anim]}>
      <Tile v={v} size={size} font={font} />
    </Animated.View>
  );
}
