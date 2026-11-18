import type { ComponentType, ReactNode } from 'react';
import { Text, type TextStyle, View } from 'react-native';

import type { Impact } from '@/lib/hooks/use-shake';
import { useT } from '@/lib/i18n';
import type { Milestone } from '@/lib/utils/milestones';
import type { Rand } from '@/lib/utils/rand';
import { tidy } from '@/lib/utils/text';
import type { Theme } from '@/lib/theme';

import type { Clock } from '@/components/celebration/engine';
import { Reveal, Slam } from '@/components/celebration/motion';
import { BigNumber } from '@/components/celebration/numbers';

export type SceneProps = {
  clock: Clock;
  m: Milestone;
  t: Theme;
  /** Stage size (the window). */
  w: number;
  h: number;
  /** Seeded with the milestone: same party every time. */
  rand: Rand;
  /** Where the number sits (its centre). */
  cx: number;
  cy: number;
};

type Scene = {
  /** Total length (ms); the party closes by itself at the end. */
  duration: number;
  /** Backdrop colour, drawn at `dim` opacity over the app. */
  bg: string;
  dim?: number;
  /** Shake the stage and buzz the phone. */
  impacts: Impact[];
  Render: ComponentType<SceneProps>;
};

export type SceneFactory = (m: Milestone, t: Theme) => Scene;

/** Font size of the big number for this many digits. */
export const numberSize = (n: number) => (String(n).length > 3 ? 104 : 128);

/**
 * The number centred on (cx, cy) in a box `box` px tall, then "questions !" and the punchline under it, revealed at `at`.
 * Children overflowing the box (reels, glows) stay visible.
 */
export function Headline({
  clock,
  m,
  t,
  at,
  children,
  ink,
  sub,
  cy,
  box,
}: {
  clock: Clock;
  m: Milestone;
  t: Theme;
  at: number;
  children: ReactNode;
  ink?: string;
  sub?: string;
  cy: number;
  box: number;
}) {
  const tr = useT();
  return (
    <>
      <View
        pointerEvents="none"
        style={{ position: 'absolute', left: 0, right: 0, top: cy - box / 2, height: box, alignItems: 'center', justifyContent: 'center' }}>
        {children}
      </View>
      <View
        pointerEvents="none"
        style={{ position: 'absolute', left: 0, right: 0, top: cy + box / 2 + 4, alignItems: 'center', paddingHorizontal: 32 }}>
        <Reveal clock={clock} at={at} style={{ alignItems: 'center' }}>
          <Text style={{ fontFamily: t.font.display, fontSize: 28, letterSpacing: -0.5, color: ink ?? t.txt }}>{m.unit ?? tr('celebration.questions')}</Text>
        </Reveal>
        <Reveal clock={clock} at={at + 140} style={{ alignItems: 'center', marginTop: 10 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 22, color: sub ?? t.mut, textAlign: 'center', maxWidth: 290 }}>
            {tidy(m.line)}
          </Text>
        </Reveal>
      </View>
    </>
  );
}

/** The usual finale: the milestone's number slams down at `slam`, its headline revealed at `at`. */
export function SlamNumber({
  clock,
  m,
  t,
  slam,
  at,
  cy,
  size,
  color,
  glow,
  ink,
  sub,
  from,
  fall,
  style,
}: {
  clock: Clock;
  m: Milestone;
  t: Theme;
  slam: number;
  at: number;
  cy: number;
  size: number;
  color: string;
  glow?: string;
  ink?: string;
  sub?: string;
  from?: number;
  fall?: number;
  style?: TextStyle;
}) {
  return (
    <Headline clock={clock} m={m} t={t} at={at} cy={cy} box={size * 1.1} ink={ink} sub={sub}>
      <Slam clock={clock} at={slam} from={from} fall={fall}>
        <BigNumber value={m.n} size={size} color={color} font={t.font.display} glow={glow} style={style} />
      </Slam>
    </Headline>
  );
}
