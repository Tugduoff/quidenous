import { useMemo } from 'react';
import { Text } from 'react-native';

import { useT } from '@/lib/i18n';

import { burst, firework, paper, type Particle, Particles, rain, sparks } from '@/components/celebration/engine';
import { Flash, Glow, Rays, Shockwave } from '@/components/celebration/effects';
import { Reveal, Slam } from '@/components/celebration/motion';
import { glowText, RollNumber } from '@/components/celebration/numbers';
import { GOLD } from '@/components/celebration/palettes';
import { Headline, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

export const mille: SceneFactory = (m, t) => {
  const slam = 1450;
  const two = m.n === 2000;
  return {
    duration: 6400,
    bg: '#120C03',
    dim: 0.97,
    impacts: [
      { at: slam, amp: 13 },
      { at: slam + 650, amp: 3 },
    ],
    Render: function Mille({ clock, w, h, rand, cx, cy }: SceneProps) {
      const tr = useT();
      const thousand = tr('celebration.thousand');
      const fw = useMemo(
        () =>
          [
            { x: w * 0.24, y: h * 0.2, at: 0, count: 24 },
            { x: w * 0.76, y: h * 0.15, at: 230, count: 24 },
            { x: w * 0.5, y: h * 0.09, at: 460, count: 24 },
          ].map((s) => firework(rand, { ...s, from: h + 10, colors: GOLD, power: 1.2 })),
        [rand, w, h],
      );
      const list = useMemo<Particle[]>(
        () => [
          ...fw.flatMap((f) => f.list),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 12, speed: [500, 1000], g: 200, k: 2.4, life: [600, 1000], look: sparks(GOLD, 3) }),
          ...rain(rand, { w, h, count: 30, from: slam + 100, to: 4200, look: paper(GOLD), life: 2800 }),
        ],
        [fw, rand, w, h, cx, cy],
      );
      const word = Math.min(98, Math.floor((w / 4) * Math.min(1, 5 / thousand.length)));
      const small = 54;
      const box = (two ? word * 0.62 : 0) + word * 1.04 + small * 1.05 + 10;
      const goldText = { color: '#FFD54A', ...glowText('#FFB300', 28) };
      return (
        <>
          {fw.slice(0, 3).map((f, i) => (
            <Flash key={i} clock={clock} at={f.boom} color="#FFD54A" peak={0.12} fall={300} />
          ))}
          <Rays clock={clock} at={slam} x={cx} y={cy} color="#FFD54A" size={820} count={18} opacity={0.32} speed={12} />
          <Glow clock={clock} at={slam} x={cx} y={cy} color="#FFB300" size={440} opacity={0.4} pulse={0.7} />
          <Flash clock={clock} at={slam} color="#FFF1B8" peak={0.55} fall={600} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color="#FFE082" size={560} width={10} dur={650} />
          <Shockwave clock={clock} at={slam + 130} x={cx} y={cy} color="#FFB300" size={820} width={5} dur={900} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={slam + 900} cy={cy} box={box} ink="#FFF6D8" sub="#D9C99A">
            <Slam clock={clock} at={slam} from={3.6} fall={240} style={{ alignItems: 'center' }}>
              {two && (
                <Text
                  style={{
                    fontFamily: t.font.display,
                    fontSize: word * 0.6,
                    lineHeight: word * 0.62,
                    letterSpacing: -1,
                    includeFontPadding: false,
                    ...goldText,
                  }}>
                  {tr('celebration.two')}
                </Text>
              )}
              <Text
                style={{
                  fontFamily: t.font.display,
                  fontSize: word,
                  lineHeight: word * 1.04,
                  letterSpacing: -2,
                  includeFontPadding: false,
                  ...goldText,
                }}>
                {thousand}
              </Text>
            </Slam>
            <Reveal clock={clock} at={slam + 450} rise={-10} style={{ marginTop: 10 }}>
              <RollNumber clock={clock} value={m.n} at={slam + 450} size={small} color="#FFF1B8" font={t.font.display} dur={700} stagger={90} />
            </Reveal>
          </Headline>
        </>
      );
    },
  };
};
