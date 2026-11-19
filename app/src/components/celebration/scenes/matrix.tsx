import { useMemo } from 'react';
import { Platform, StyleSheet, Text, View, type ViewStyle } from 'react-native';
import Animated, { Extrapolation, interpolate, useAnimatedStyle } from 'react-native-reanimated';

import { useT } from '@/lib/i18n';
import { clamp01 } from '@/lib/utils/easing';

import { burst, type Clock, type Particle, Particles } from '@/components/celebration/engine';
import { Flash, Glow, Scanlines, Shockwave, SweepBand } from '@/components/celebration/effects';
import { Reveal } from '@/components/celebration/motion';
import { glowText } from '@/components/celebration/numbers';
import { Headline, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

const MONO = Platform.OS === 'web' ? 'ui-monospace, Menlo, Consolas, monospace' : 'monospace';

const HEX = '0123456789ABCDEF';

function RainColumn({
  clock,
  x,
  y0,
  speed,
  text,
  head,
  h,
  opacity,
  size,
}: {
  clock: Clock;
  x: number;
  y0: number;
  speed: number;
  text: string;
  head: string;
  h: number;
  opacity: number;
  size: number;
}) {
  const lineH = Math.round(size * 1.2);
  const colH = (text.length / 2 + 1) * lineH;
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const y = ((y0 + (speed * c) / 1000) % (h + colH)) - colH;
    return { opacity: clamp01(c / 500) * opacity, transform: [{ translateY: y }] };
  });
  return (
    <Animated.View style={[{ position: 'absolute', left: x, top: 0 }, style]}>
      <Text style={{ fontFamily: MONO, fontSize: size, lineHeight: lineH, color: '#22C55E', textAlign: 'center', includeFontPadding: false }}>
        {text}
        <Text style={{ color: '#D8FFE0', ...glowText('#3DFF6E', 8) }}>{head}</Text>
      </Text>
    </Animated.View>
  );
}

function DecodeChar({ clock, strip, start, land, size, w }: { clock: Clock; strip: string[]; start: number; land: number; size: number; w: number }) {
  const lineH = Math.round(size * 1.08);
  const n = strip.length;
  const style = useAnimatedStyle(() => {
    const p = clamp01((clock.value - start) / (land - start));
    return { opacity: clock.value >= start ? 1 : 0, transform: [{ translateY: -Math.floor(p * (n - 1)) * lineH }] };
  });
  const lock = useAnimatedStyle(() => ({ opacity: interpolate(clock.value, [land, land + 60, land + 400], [0, 1, 0], Extrapolation.CLAMP) }));
  return (
    <View style={{ width: w, height: lineH, overflow: 'hidden' }}>
      <Animated.View style={style}>
        {strip.map((ch, i) => (
          <Text
            key={i}
            style={{
              height: lineH,
              fontFamily: MONO,
              fontWeight: '800',
              fontSize: size,
              lineHeight: lineH,
              textAlign: 'center',
              color: i === n - 1 ? '#C9FFD6' : '#3DFF6E',
              includeFontPadding: false,
            }}>
            {ch}
          </Text>
        ))}
      </Animated.View>
      <Animated.View
        pointerEvents="none"
        style={[StyleSheet.absoluteFill, { backgroundColor: '#3DFF6E' }, lock, { mixBlendMode: 'screen' } as ViewStyle]}
      />
    </View>
  );
}

function Flicker({ clock }: { clock: Clock }) {
  const style = useAnimatedStyle(() => {
    const c = clock.value;
    const n = Math.sin(c * 0.091) * Math.sin(c * 0.037 + 1.3);
    return { opacity: 0.05 + 0.06 * Math.max(0, n) };
  });
  return <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: '#000000' }, style]} />;
}

export const matrix: SceneFactory = (m, t) => {
  const start = 500;
  const lands = [1300, 1550, 1800, 2050];
  const done = lands[3];
  return {
    duration: 6000,
    bg: '#000000',
    dim: 0.98,
    impacts: [...lands.slice(0, 3).map((at) => ({ at, amp: 1.5 })), { at: done, amp: 6 }],
    Render: function Matrix({ clock, w, h, rand, cx, cy }: SceneProps) {
      const tr = useT();
      const glyph = 15;
      const cols = useMemo(() => {
        const step = 19;
        return Array.from({ length: Math.ceil(w / step) }, (_, i) => {
          const len = Math.floor(rand.range(14, 30));
          const chars = Array.from({ length: len }, () => rand.pick([...HEX, ...'01']));
          return {
            x: i * step + 2,
            y0: rand.range(0, h),
            speed: rand.range(110, 300),
            text: chars.join('\n') + '\n',
            head: rand.pick([...HEX]),
            opacity: rand.range(0.35, 0.85),
          };
        });
      }, [rand, w, h]);
      const size = 92;
      const charW = Math.round(size * 0.64);
      const strips = useMemo(
        () =>
          String(m.n)
            .split('')
            .map((ch, i) => [...Array.from({ length: 14 + i * 4 }, () => rand.pick([...HEX])), ch]),
        [rand],
      );
      const list = useMemo<Particle[]>(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: done,
            count: 34,
            speed: [250, 750],
            g: 600,
            k: 1.8,
            life: [1000, 1600],
            fadeOut: 500,
            look: (r) => ({
              shape: 'text',
              glyph: r.pick([...HEX]),
              font: MONO,
              color: r.pick(['#3DFF6E', '#22C55E', '#C9FFD6']),
              w: 16,
              h: 18,
              spin: r.range(-180, 180),
            }),
          }),
        [rand, cx, cy],
      );
      const box = size * 1.08 + 30;
      return (
        <>
          {cols.map((c, i) => (
            <RainColumn key={i} clock={clock} h={h} size={glyph} {...c} />
          ))}
          <Glow clock={clock} at={0} x={cx} y={cy + 30} color="#000000" size={680} opacity={1} />
          <Glow clock={clock} at={0} x={cx} y={cy} color="#000000" size={420} opacity={1} />
          <Glow clock={clock} at={start} x={cx} y={cy} color="#3DFF6E" size={300} opacity={0.12} />
          <Glow clock={clock} at={done} x={cx} y={cy} color="#3DFF6E" size={380} opacity={0.22} pulse={0.6} />
          <Flash clock={clock} at={done} color="#3DFF6E" peak={0.3} fall={380} />
          <Shockwave clock={clock} at={done} x={cx} y={cy} color="#3DFF6E" size={520} width={4} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={done + 300} cy={cy} box={box} ink="#E6FFEC" sub="#7FD99A">
            <Reveal clock={clock} at={done + 80} rise={6} dur={200}>
              <Text
                style={{
                  fontFamily: MONO,
                  fontSize: 13,
                  lineHeight: 18,
                  letterSpacing: 4,
                  color: '#3DFF6E',
                  marginBottom: 10,
                  ...glowText('#3DFF6E', 8),
                }}>
                {tr('celebration.access')}
              </Text>
            </Reveal>
            <View style={{ flexDirection: 'row' }}>
              {strips.map((s, i) => (
                <DecodeChar key={i} clock={clock} strip={s} start={start} land={lands[i]} size={size} w={charW} />
              ))}
            </View>
          </Headline>
          <SweepBand clock={clock} w={w} h={h} period={2600} size={160} reach={160} peak={0.5} color="#3DFF6E" />
          <Scanlines w={w} h={h} color="#000000" opacity={0.35} thick={1.5} />
          <Flicker clock={clock} />
        </>
      );
    },
  };
};
