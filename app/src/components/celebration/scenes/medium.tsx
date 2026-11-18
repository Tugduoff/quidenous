import { useMemo } from 'react';

import { burst, firework, paper, Particles, rain } from '@/components/celebration/engine';
import { Flash, Glow, Rays, Shockwave } from '@/components/celebration/effects';
import { GOLD, party } from '@/components/celebration/palettes';
import { numberSize, type SceneFactory, type SceneProps, SlamNumber } from '@/components/celebration/scene';

export const MEDIUM_SLAM = 1350;

export const medium: SceneFactory = (m, t) => {
  const gold = m.theme === 'gold';
  const slam = MEDIUM_SLAM;
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
          <SlamNumber
            clock={clock}
            m={m}
            t={t}
            slam={slam}
            at={slam + 250}
            cy={cy}
            size={size}
            color={ink}
            glow={gold ? '#FFB300' : undefined}
            ink={gold ? '#FFF6D8' : undefined}
            sub={gold ? '#D9C99A' : undefined}
          />
        </>
      );
    },
  };
};
