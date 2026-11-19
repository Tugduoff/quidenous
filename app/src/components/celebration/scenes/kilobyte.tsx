import { useMemo } from 'react';
import { Text, View } from 'react-native';
import Animated, { useAnimatedStyle } from 'react-native-reanimated';

import { useT } from '@/lib/i18n';

import { burst, type Clock, Particles } from '@/components/celebration/engine';
import { Flash, Scanlines, Shockwave } from '@/components/celebration/effects';
import { ABS } from '@/components/celebration/layout';
import { FadeAway, Window } from '@/components/celebration/motion';
import { PIXEL } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

export const kilobyte: SceneFactory = (m, t) => {
  const segs = 16;
  const load: [number, number] = [300, 1800];
  const slam = 2150;
  return {
    duration: 5400,
    bg: '#0F0F1B',
    dim: 0.98,
    impacts: [
      { at: load[1], amp: 2 },
      { at: slam, amp: 8 },
    ],
    Render: function Pixels({ clock, w, h, rand, cx, cy }: SceneProps) {
      const blocks = useMemo(
        () =>
          Array.from({ length: 46 }, () => {
            const size = rand.pick([8, 12, 12, 16]);
            return {
              x: Math.round(rand.range(0, w - size) / 8) * 8,
              y0: -size - Math.round(rand.range(0, 6)) * 16,
              at: rand.range(0, 3600),
              step: rand.pick([70, 90, 110]),
              size,
              color: rand.pick(PIXEL),
            };
          }),
        [rand, w],
      );
      const tr = useT();
      const list = useMemo(
        () =>
          burst(rand, {
            x: cx,
            y: cy,
            at: slam,
            count: 36,
            speed: [350, 900],
            g: 1000,
            k: 1.6,
            life: [1100, 1700],
            look: (r) => {
              const s = r.pick([6, 8, 10]);
              return { shape: 'rect', color: r.pick(PIXEL), w: s, h: s, rot: 0, spin: 0 };
            },
          }),
        [rand, cx, cy],
      );
      const barW = 240;
      const barY = cy - 150;
      const segW = (barW - 6 - 2 * (segs + 1)) / segs;
      const size = numberSize(m.n);
      return (
        <>
          <Scanlines w={w} h={h} color="#FFFFFF" opacity={0.035} thick={1.5} />
          {blocks.map((b, i) => (
            <StepBlock key={i} clock={clock} b={b} h={h} />
          ))}
          <View pointerEvents="none" style={{ position: 'absolute', left: cx - barW / 2, top: barY, width: barW }}>
            <View style={{ height: 30, borderWidth: 3, borderColor: '#FFFFFF', flexDirection: 'row', padding: 2, gap: 2 }}>
              {Array.from({ length: segs }, (_, i) => (
                <Segment
                  key={i}
                  clock={clock}
                  at={load[0] + ((load[1] - load[0]) * (i + 1)) / segs}
                  blinkFrom={load[1]}
                  w={segW}
                  color={i < segs - 1 ? '#3DFF6E' : '#FFE600'}
                />
              ))}
            </View>
            <View style={{ height: 22, marginTop: 8 }}>
              <FadeAway clock={clock} from={load[1]} to={load[1] + 1} style={{ position: 'absolute', left: 0, right: 0 }}>
                <Text style={{ fontFamily: 'monospace', fontSize: 14, color: '#9C9CC0', textAlign: 'center', letterSpacing: 2 }}>{tr('celebration.loading')}</Text>
              </FadeAway>
              <Window clock={clock} from={load[1]} to={1e9} style={{ position: 'absolute', left: 0, right: 0 }}>
                <Text style={{ fontFamily: 'monospace', fontSize: 14, color: '#FFE600', textAlign: 'center', letterSpacing: 2 }}>{tr('celebration.loaded')}</Text>
              </Window>
            </View>
          </View>
          <Flash clock={clock} at={slam} color="#FFFFFF" peak={0.35} rise={0} fall={180} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#05D9E8" size={460} width={6} />
          <Particles clock={clock} list={list} />
          <SlamNumber
            clock={clock}
            m={m}
            t={t}
            slam={slam}
            at={slam + 280}
            cy={cy}
            size={size}
            color="#FFE600"
            ink="#FFFFFF"
            sub="#9C9CC0"
            from={2.4}
            style={{ letterSpacing: 2, textShadowColor: '#FF2A6D', textShadowOffset: { width: 6, height: 6 }, textShadowRadius: 0 }}
          />
        </>
      );
    },
  };
};

function Segment({ clock, at, blinkFrom, w, color }: { clock: Clock; at: number; blinkFrom: number; w: number; color: string }) {
  const anim = useAnimatedStyle(() => {
    const c = clock.value;
    if (c < at) return { opacity: 0 };
    const k = Math.floor((c - blinkFrom) / 110);
    return { opacity: c >= blinkFrom && k < 6 && k % 2 === 0 ? 0.25 : 1 };
  });
  return <Animated.View style={[{ width: w, height: '100%', backgroundColor: color }, anim]} />;
}

function StepBlock({
  clock,
  b,
  h,
}: {
  clock: Clock;
  b: { x: number; y0: number; at: number; step: number; size: number; color: string };
  h: number;
}) {
  const { x, y0, at, step, size } = b;
  const anim = useAnimatedStyle(() => {
    const c = clock.value - at;
    if (c < 0) return { opacity: 0, transform: [{ translateX: x }, { translateY: -50 }] };
    const y = y0 + Math.floor(c / step) * 16;
    return { opacity: y > h ? 0 : 1, transform: [{ translateX: x }, { translateY: y }] };
  });
  return <Animated.View pointerEvents="none" style={[ABS, { width: size, height: size, backgroundColor: b.color }, anim]} />;
}
