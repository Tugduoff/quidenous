/** Every 50: two confetti cannons, the number rolls up, a little pop when it lands. About 2.5 s. */
import { useMemo } from 'react';

import { burst, cannons, paper, Particles } from '@/components/celebration/engine';
import { Shockwave } from '@/components/celebration/effects';
import { RollNumber } from '@/components/celebration/numbers';
import { party } from '@/components/celebration/palettes';
import { Headline, numberSize, type SceneFactory, type SceneProps } from '@/components/celebration/scene';

const ROLL_START = 150;
const ROLL_DUR = 820;
const ROLL_STAGGER = 120;

/** When the rolled-up number lands: its last digit settles. */
export const smallLanding = (n: number) => ROLL_START + ROLL_DUR + (String(n).length - 1) * ROLL_STAGGER;

export const small: SceneFactory = (m, t) => {
  const land = smallLanding(m.n);
  return {
    duration: 2700,
    bg: t.bg,
    impacts: [{ at: land, amp: 3 }],
    Render: function Small({ clock, w, h, rand, cx, cy }: SceneProps) {
      const colors = party(t);
      const list = useMemo(
        () => [
          ...cannons(rand, {
            w,
            y: h + 10,
            at: 0,
            delay: 60,
            count: 26,
            dir: -62,
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
            <RollNumber clock={clock} value={m.n} at={ROLL_START} size={size} color={t.accInk} font={t.font.display} dur={ROLL_DUR} stagger={ROLL_STAGGER} />
          </Headline>
        </>
      );
    },
  };
};
