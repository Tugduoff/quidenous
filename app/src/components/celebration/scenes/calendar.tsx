/** 365: tear-off calendar — the last days of December fly off. */
import { useMemo } from 'react';
import { Text, View } from 'react-native';

import { useT } from '@/lib/i18n';

import { burst, paper, Particles } from '@/components/celebration/engine';
import { Flash, Rays, Shockwave } from '@/components/celebration/effects';
import { centred } from '@/components/celebration/layout';
import { Keyed, Thrown } from '@/components/celebration/motion';
import { party } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

function Page({ day, font, bold }: { day: number; font: string; bold: string }) {
  const tr = useT();
  return (
    <View
      style={{ width: 140, height: 156, borderRadius: 18, backgroundColor: '#FFFFFF', borderWidth: 1, borderColor: '#00000018', overflow: 'hidden' }}>
      <View style={{ height: 42, backgroundColor: '#E5484D', alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ fontFamily: bold, fontSize: 17, letterSpacing: 2, color: '#FFFFFF' }}>{tr('celebration.december')}</Text>
      </View>
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ fontFamily: font, fontSize: 76, lineHeight: 84, letterSpacing: -3, color: '#121216' }}>{day}</Text>
      </View>
    </View>
  );
}

export const calendar: SceneFactory = (m, t) => {
  const first = 250;
  const step = 95;
  const pages = 12;
  const slam = first + pages * step + 200;
  return {
    duration: 4000,
    bg: t.bg,
    impacts: [
      { at: first, amp: 2 },
      { at: slam, amp: 8 },
    ],
    Render: function Calendar({ clock, rand, cx, cy }: SceneProps) {
      const torn = useMemo(
        () =>
          Array.from({ length: pages }, (_, i) => {
            const s = rand.sign();
            return {
              day: 19 + i,
              p: {
                x: cx,
                y: cy,
                vx: s * rand.range(320, 700),
                vy: -rand.range(550, 950),
                g: 2800,
                k: 0.4,
                spin: s * rand.range(300, 800),
                rot: 0,
                at: first + i * step,
                w: 140,
                h: 156,
              },
            };
          }).reverse(),
        [rand, cx, cy],
      );
      const list = useMemo(
        () => burst(rand, { x: cx, y: cy, at: slam, count: 36, speed: [400, 900], g: 900, k: 1.8, life: [1300, 1900], look: paper(party(t)) }),
        [rand, cx, cy],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Rays clock={clock} at={slam} x={cx} y={cy} color={t.acc} opacity={0.16} />
          <Keyed clock={clock} style={centred(cx, cy, 140, 156)} k={{ t: [0, 180, slam - 140, slam - 20], s: [0.4, 1, 1, 0.5], o: [0, 1, 1, 0] }}>
            <Page day={31} font={t.font.display} bold={t.font.bold} />
          </Keyed>
          {torn.map((p) => (
            <Keyed key={p.day} clock={clock} k={{ t: [0, 180], o: [0, 1] }}>
              <Thrown clock={clock} p={p.p}>
                <Page day={p.day} font={t.font.display} bold={t.font.bold} />
              </Thrown>
            </Keyed>
          ))}
          {/* The binder rings stay with the block. */}
          <Keyed clock={clock} style={centred(cx, cy - 78, 104, 20)} k={{ t: [0, 180, slam - 140, slam - 20], o: [0, 1, 1, 0] }}>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingHorizontal: 20 }}>
              <View style={{ width: 12, height: 20, borderRadius: 6, backgroundColor: '#3A3A44' }} />
              <View style={{ width: 12, height: 20, borderRadius: 6, backgroundColor: '#3A3A44' }} />
            </View>
          </Keyed>
          <Flash clock={clock} at={slam} color={t.acc} peak={0.3} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color={t.acc} size={480} width={7} />
          <Particles clock={clock} list={list} />
          <SlamNumber clock={clock} m={m} t={t} slam={slam} at={slam + 250} cy={cy} size={size} color={t.accInk} />
        </>
      );
    },
  };
};
