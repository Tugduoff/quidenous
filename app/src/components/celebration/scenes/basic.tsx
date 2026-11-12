/** The two generic parties: every 50 (small) and round hundreds / fun numbers without a scene of their own (medium). */
import { useMemo } from 'react';

import { BigNumber, burst, Flash, firework, Glow, paper, Particles, Rays, RollNumber, Shockwave, Slam, rain } from '../engine';
import { GOLD, Headline, numberSize, party, type SceneFactory, type SceneProps } from '../scene';

/** Every 50: two confetti cannons, the number rolls up, a little pop when it lands. About 2.5 s. */
export const small: SceneFactory = (m, t) => {
  const digits = String(m.n).length;
  const land = 150 + 820 + (digits - 1) * 120;
  return {
    duration: 2700,
    bg: t.bg,
    impacts: [{ at: land, amp: 3 }],
    Render: function Small({ clock, w, h, rand, cx, cy }: SceneProps) {
      const colors = party(t);
      const list = useMemo(
        () => [
          ...burst(rand, {
            x: -10,
            y: h + 10,
            at: 0,
            count: 26,
            dir: -62,
            spread: 26,
            speed: [1150, 1800],
            g: 950,
            k: 1.3,
            life: [2000, 2500],
            look: paper(colors),
          }),
          ...burst(rand, {
            x: w + 10,
            y: h + 10,
            at: 60,
            count: 26,
            dir: -118,
            spread: 26,
            speed: [1150, 1800],
            g: 950,
            k: 1.3,
            life: [2000, 2500],
            look: paper(colors),
          }),
          ...burst(rand, { x: cx, y: cy, at: land, count: 18, speed: [250, 600], g: 700, k: 2, life: [900, 1300], look: paper(colors) }),
        ],
        [rand, w, h, cx, cy, colors],
      );
      const size = numberSize(m.n);
      return (
        <>
          <Shockwave clock={clock} at={land} x={cx} y={cy} color={t.acc} size={300} width={4} />
          <Particles clock={clock} list={list} />
          <Headline clock={clock} m={m} t={t} at={land - 100} cy={cy} box={size * 1.1}>
            <RollNumber clock={clock} value={m.n} at={150} size={size} color={t.accInk} font={t.font.display} dur={820} stagger={120} />
          </Headline>
        </>
      );
    },
  };
};

/** Round hundreds and fun numbers: fireworks, then the number slams down with a flash, rays and a confetti shower. About 4 s. */
export const medium: SceneFactory = (m, t) => {
  const gold = m.theme === 'gold';
  const slam = 1350;
  return {
    duration: 4200,
    bg: gold ? '#0E0B05' : t.bg,
    dim: gold ? 0.97 : 0.96,
    impacts: [
      { at: slam, amp: 9 },
      { at: 2050, amp: 2 },
    ],
    Render: function Medium({ clock, w, h, rand, cx, cy }: SceneProps) {
      const colors = gold ? GOLD : party(t);
      const fw = useMemo(() => {
        const shots = [
          { x: w * 0.25, y: h * 0.2, at: 0 },
          { x: w * 0.75, y: h * 0.16, at: 260 },
          { x: w * 0.5, y: h * 0.1, at: 520 },
          { x: w * 0.2, y: h * 0.26, at: 1850 },
          { x: w * 0.8, y: h * 0.24, at: 2150 },
        ];
        return shots.map((s) => firework(rand, { ...s, from: h + 10, colors, count: 24 }));
      }, [rand, w, h, colors]);
      const list = useMemo(
        () => [
          ...fw.flatMap((f) => f.list),
          ...burst(rand, { x: cx, y: cy, at: slam, count: 34, speed: [400, 900], g: 900, k: 1.8, life: [1400, 2000], look: paper(colors) }),
          ...rain(rand, { w, h, count: 34, from: slam + 150, to: 3200, look: paper(colors), life: 2600 }),
        ],
        [fw, rand, w, h, cx, cy, colors],
      );
      const size = numberSize(m.n);
      const ink = gold ? '#FFD54A' : t.accInk;
      return (
        <>
          {fw.slice(0, 3).map((f, i) => (
            <Flash key={i} clock={clock} at={f.boom} color={colors[i % colors.length]} peak={0.08} fall={300} />
          ))}
          <Rays clock={clock} at={slam} x={cx} y={cy} color={gold ? '#FFD54A' : t.acc} opacity={gold ? 0.3 : 0.18} />
          <Glow clock={clock} at={slam} x={cx} y={cy} color={gold ? '#FFB300' : t.acc} size={340} opacity={0.35} pulse={0.8} />
          <Flash clock={clock} at={slam} color={gold ? '#FFE082' : t.acc} peak={0.28} fall={340} />
          <Shockwave clock={clock} at={slam} x={cx} y={cy} color={gold ? '#FFE082' : t.acc} size={520} width={8} dur={700} />
          <Particles clock={clock} list={list} />
          <Headline
            clock={clock}
            m={m}
            t={t}
            at={slam + 250}
            cy={cy}
            box={size * 1.1}
            ink={gold ? '#FFF6D8' : undefined}
            sub={gold ? '#D9C99A' : undefined}>
            <Slam clock={clock} at={slam}>
              <BigNumber value={m.n} size={size} color={ink} font={t.font.display} glow={gold ? '#FFB300' : undefined} />
            </Slam>
          </Headline>
        </>
      );
    },
  };
};
