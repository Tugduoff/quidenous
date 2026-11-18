/** 777: a slot machine that lands on 7-7-7, then a coin fountain. */
import { useMemo } from 'react';
import { Platform, StyleSheet, Text, View, type ViewStyle } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';
import Svg, { Circle, Defs, LinearGradient, Path, Polygon, Rect, Stop } from 'react-native-svg';

import { useT } from '@/lib/i18n';
import { backOut, clamp01 } from '@/lib/utils/easing';
import type { Rand } from '@/lib/utils/rand';

import { burst, type Clock, paper, type Particle, Particles, rain, sparks } from '@/components/celebration/engine';
import { Flash, Glow, Rays, Shockwave } from '@/components/celebration/effects';
import { Keyed, Pop, Slam } from '@/components/celebration/motion';
import { glowText } from '@/components/celebration/numbers';
import { GOLD } from '@/components/celebration/palettes';
import { Headline, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

type Sym = 'seven' | 'cherry' | 'bell' | 'bar' | 'diamond';
const CELL_W = 84;
const CELL_H = 104;

function SlotSymbol({ kind, font }: { kind: Sym; font: string }) {
  switch (kind) {
    case 'seven':
      return (
        <Text
          style={{
            fontFamily: font,
            fontSize: 84,
            lineHeight: CELL_H,
            color: '#FF2D55',
            includeFontPadding: false,
            textShadowColor: '#7A0018',
            textShadowOffset: { width: 3, height: 4 },
            textShadowRadius: 0,
          }}>
          7
        </Text>
      );
    case 'cherry':
      return (
        <Svg width={60} height={60} viewBox="0 0 60 60">
          <Path d="M20 40 C 24 22, 32 12, 44 8 M40 42 C 40 26, 42 16, 44 8" stroke="#2E8B3A" strokeWidth={3.5} fill="none" strokeLinecap="round" />
          <Path d="M44 8 C 50 4, 56 8, 54 14 C 50 14, 46 12, 44 8 Z" fill="#3DBA4E" />
          <Circle cx={19} cy={43} r={12} fill="#E0102F" />
          <Circle cx={40} cy={44} r={12} fill="#C70A28" />
          <Circle cx={15} cy={39} r={3.5} fill="#FFFFFF" opacity={0.7} />
          <Circle cx={36} cy={40} r={3.5} fill="#FFFFFF" opacity={0.7} />
        </Svg>
      );
    case 'bell':
      return (
        <Svg width={60} height={60} viewBox="0 0 60 60">
          <Path
            d="M30 6 C 18 6, 14 18, 14 30 C 14 38, 10 42, 7 46 L 53 46 C 50 42, 46 38, 46 30 C 46 18, 42 6, 30 6 Z"
            fill="#FFC21A"
            stroke="#B37400"
            strokeWidth={2.5}
          />
          <Circle cx={30} cy={51} r={5} fill="#B37400" />
          <Path d="M22 16 C 19 20, 18 26, 18 32" stroke="#FFF1B8" strokeWidth={3} fill="none" strokeLinecap="round" />
        </Svg>
      );
    case 'diamond':
      return (
        <Svg width={56} height={56} viewBox="0 0 56 56">
          <Polygon points="28,50 4,20 14,8 42,8 52,20" fill="#3DD6F5" stroke="#0A7FA8" strokeWidth={2.5} />
          <Polygon points="4,20 52,20 28,50" fill="#1FB3DB" />
          <Polygon points="14,8 22,20 28,8 34,20 42,8" fill="#9BEFFF" />
        </Svg>
      );
    case 'bar':
      return (
        <View
          style={{
            width: 66,
            height: 34,
            borderRadius: 6,
            backgroundColor: '#15121C',
            borderWidth: 2.5,
            borderColor: '#FFC21A',
            alignItems: 'center',
            justifyContent: 'center',
          }}>
          <Text style={{ fontFamily: font, fontSize: 20, lineHeight: 24, color: '#FFFFFF', letterSpacing: 1, includeFontPadding: false }}>BAR</Text>
        </View>
      );
  }
}

/** One reel: a strip of symbols scrolling up, landing on a 7 (with a small bounce past it). */
function SlotReel({ clock, strip, start, land, font }: { clock: Clock; strip: Sym[]; start: number; land: number; font: string }) {
  // The 7 is second to last: the bounce overshoots onto the one after it.
  const end = (strip.length - 2) * CELL_H;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - start) / (land - start));
    return { transform: [{ translateY: -backOut(p, 1.1) * end }] };
  });
  return (
    <View style={{ width: CELL_W, height: CELL_H, overflow: 'hidden', borderRadius: 10, backgroundColor: '#FFF8E7' }}>
      <Animated.View style={style}>
        {strip.map((k, i) => (
          <View key={i} style={{ width: CELL_W, height: CELL_H, alignItems: 'center', justifyContent: 'center' }}>
            <SlotSymbol kind={k} font={font} />
          </View>
        ))}
      </Animated.View>
      {/* Cylinder shading. */}
      <View pointerEvents="none" style={StyleSheet.absoluteFill}>
        <Svg width={CELL_W} height={CELL_H}>
          <Defs>
            <LinearGradient id="reelshade" x1="0" y1="0" x2="0" y2="1">
              <Stop offset="0" stopColor="#2A145C" stopOpacity={0.75} />
              <Stop offset="0.3" stopColor="#2A145C" stopOpacity={0} />
              <Stop offset="0.7" stopColor="#2A145C" stopOpacity={0} />
              <Stop offset="1" stopColor="#2A145C" stopOpacity={0.75} />
            </LinearGradient>
          </Defs>
          <Rect x={0} y={0} width={CELL_W} height={CELL_H} fill="url(#reelshade)" />
        </Svg>
      </View>
    </View>
  );
}

/** A marquee bulb: chases slowly while the reels spin, then flashes fast on the jackpot. */
function Bulb({ clock, x, y, parity, jackpot }: { clock: Clock; x: number; y: number; parity: number; jackpot: number }) {
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const step = c < jackpot ? Math.floor(c / 220) : Math.floor(c / 110);
    const on = (step + parity) % 2 === 0;
    return { opacity: on ? 1 : 0.25 };
  });
  return (
    <Animated.View
      style={[{ position: 'absolute', left: x - 4, top: y - 4, width: 8, height: 8, borderRadius: 4, backgroundColor: '#FFF1B8' }, glowShadow, style]}
    />
  );
}
const glowShadow: ViewStyle = Platform.OS === 'web' ? ({ boxShadow: '0 0 8px #FFD54A' } as ViewStyle) : {};

const COIN = (
  <Svg width={24} height={24} viewBox="0 0 24 24">
    <Circle cx={12} cy={12} r={11.5} fill="#C98A0B" />
    <Circle cx={12} cy={12} r={9.5} fill="#FFD54A" />
    <Circle cx={12} cy={12} r={6} fill="none" stroke="#E8A317" strokeWidth={1.6} />
    <Path d="M8 7 A 6 6 0 0 1 14 5.5" stroke="#FFF6D0" strokeWidth={1.6} fill="none" strokeLinecap="round" />
  </Svg>
);
const coin = (r: Rand) => ({
  shape: 'node' as const,
  node: COIN,
  w: 24,
  h: 24,
  color: '#FFD54A',
  flip: r.range(7, 15),
  rot: r.range(-30, 30),
  spin: r.range(-90, 90),
});

export const jackpot: SceneFactory = (m, t) => {
  const start = 250;
  const lands = [1500, 2050, 2650];
  const win = lands[2];
  return {
    duration: 6200,
    bg: '#120A2A',
    dim: 0.97,
    impacts: [
      { at: lands[0], amp: 3 },
      { at: lands[1], amp: 3 },
      { at: win, amp: 8 },
      { at: win + 120, amp: 4 },
    ],
    Render: function Jackpot({ clock, w, h, rand, cx, cy }: SceneProps) {
      const tr = useT();
      const strips = useMemo(() => {
        const others: Sym[] = ['cherry', 'bell', 'bar', 'diamond', 'cherry', 'bell', 'bar', 'seven'];
        return lands.map((_, i) => [...Array.from({ length: 12 + i * 5 }, () => rand.pick(others)), 'seven' as Sym, rand.pick(others)]);
      }, [rand]);
      const machineW = 3 * CELL_W + 2 * 8 + 2 * 16;
      const machineH = CELL_H + 2 * 16;
      const bulbs = useMemo(() => {
        const out: { x: number; y: number }[] = [];
        const nx = 9;
        for (let i = 0; i < nx; i++) {
          const x = 10 + (i * (machineW - 20)) / (nx - 1);
          out.push({ x, y: 7 }, { x, y: machineH - 7 });
        }
        return out;
      }, [machineW, machineH]);
      const list = useMemo<Particle[]>(
        () => [
          ...burst(rand, {
            x: cx,
            y: cy + machineH / 2,
            at: win + 40,
            count: 34,
            dir: -90,
            spread: 70,
            speed: [900, 1500],
            g: 1500,
            k: 0.4,
            life: [2200, 2800],
            fadeOut: 300,
            look: coin,
          }),
          ...burst(rand, {
            x: cx,
            y: cy + machineH / 2,
            at: win + 650,
            count: 16,
            dir: -90,
            spread: 50,
            speed: [800, 1250],
            g: 1500,
            k: 0.4,
            life: [2000, 2400],
            fadeOut: 300,
            look: coin,
          }),
          ...burst(rand, { x: cx, y: cy, at: win, count: 18, speed: [500, 900], g: 300, k: 2.4, life: [600, 900], look: sparks(GOLD, 3) }),
          ...rain(rand, { w, h, count: 28, from: win + 200, to: win + 2400, look: paper([...GOLD, '#FF2D55', '#B48CFF']), life: 2800 }),
        ],
        [rand, w, h, cx, cy, machineH],
      );
      const font = t.font.display;
      return (
        <>
          <Rays clock={clock} at={win} x={cx} y={cy} color="#FFD54A" size={760} count={16} opacity={0.3} speed={24} />
          <Glow clock={clock} at={win} x={cx} y={cy} color="#FFB300" size={460} opacity={0.45} pulse={1.4} />
          <Flash clock={clock} at={win} color="#FFE082" peak={0.6} fall={500} />
          <Shockwave clock={clock} at={win} x={cx} y={cy} color="#FFE082" size={620} width={8} />
          <Headline clock={clock} m={m} t={t} at={win + 300} cy={cy} box={machineH + 12} ink="#FFFFFF" sub="#C9BEEA">
            <Slam
              clock={clock}
              at={win + 120}
              from={2.4}
              fall={160}
              style={{ position: 'absolute', top: -62, left: 0, right: 0, alignItems: 'center' }}>
              <Text
                style={{
                  fontFamily: font,
                  fontSize: 40,
                  lineHeight: 46,
                  letterSpacing: 1,
                  color: '#FFD54A',
                  includeFontPadding: false,
                  ...glowText('#FFB300', 18),
                }}>
                {tr('celebration.jackpot')}
              </Text>
            </Slam>
            <Pop clock={clock} at={0}>
              <Keyed clock={clock} k={{ t: [win, win + 90, win + 220, win + 360], s: [1, 1.09, 0.98, 1] }}>
                <View
                  style={{
                    width: machineW,
                    height: machineH,
                    borderRadius: 22,
                    backgroundColor: '#2A145C',
                    borderWidth: 3,
                    borderColor: '#FFD54A',
                    flexDirection: 'row',
                    alignItems: 'center',
                    justifyContent: 'center',
                    gap: 8,
                  }}>
                  {strips.map((s, i) => (
                    <SlotReel key={i} clock={clock} strip={s} start={start + i * 60} land={lands[i]} font={font} />
                  ))}
                  {bulbs.map((b, i) => (
                    <Bulb key={i} clock={clock} x={b.x - 3} y={b.y - 3} parity={(Math.floor(i / 2) + (i % 2)) % 2} jackpot={win} />
                  ))}
                  {/* The pay line. */}
                  <View
                    pointerEvents="none"
                    style={{
                      position: 'absolute',
                      left: 4,
                      top: machineH / 2 - 9,
                      width: 0,
                      height: 0,
                      borderTopWidth: 6,
                      borderBottomWidth: 6,
                      borderLeftWidth: 9,
                      borderTopColor: 'transparent',
                      borderBottomColor: 'transparent',
                      borderLeftColor: '#FF2D55',
                    }}
                  />
                  <View
                    pointerEvents="none"
                    style={{
                      position: 'absolute',
                      right: 4,
                      top: machineH / 2 - 9,
                      width: 0,
                      height: 0,
                      borderTopWidth: 6,
                      borderBottomWidth: 6,
                      borderRightWidth: 9,
                      borderTopColor: 'transparent',
                      borderBottomColor: 'transparent',
                      borderRightColor: '#FF2D55',
                    }}
                  />
                </View>
              </Keyed>
            </Pop>
          </Headline>
          <Particles clock={clock} list={list} />
        </>
      );
    },
  };
};
