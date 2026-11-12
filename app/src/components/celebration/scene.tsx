import type { ComponentType, ReactNode } from 'react';
import { Text, View } from 'react-native';

import type { Milestone } from '@/lib/milestones';
import { type Theme, themes } from '@/lib/theme';

import { type Clock, type Impact, type Rand, Reveal } from './engine';

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

export type Scene = {
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

/** Confetti colours around the room's accent. */
export const party = (t: Theme) =>
  [t.acc, '#FF4FA3', '#5AC8FA', '#FFB547', '#B48CFF', t.bg === themes.light.bg ? '#121216' : '#FFFFFF'].filter((c, i, xs) => xs.indexOf(c) === i);
export const GOLD = ['#FFD54A', '#FFB300', '#FFF1B8', '#E8A317', '#FFE082'] as const;

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
          <Text style={{ fontFamily: t.font.display, fontSize: 28, letterSpacing: -0.5, color: ink ?? t.txt }}>questions !</Text>
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

/** French punctuation: the space before ? ! : ; never breaks, so a "?" can't wrap onto a line of its own. */
export const tidy = (s: string) => s.replace(/ ([?!:;])/g, '\u00A0$1');
