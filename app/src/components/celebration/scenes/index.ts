import { type Milestone, tier } from '@/lib/utils/milestones';
import type { Theme } from '@/lib/theme';

import type { SceneFactory } from '@/components/celebration/scene';
import { bastille } from '@/components/celebration/scenes/bastille';
import { beer } from '@/components/celebration/scenes/beer';
import { bowling } from '@/components/celebration/scenes/bowling';
import { calendar } from '@/components/celebration/scenes/calendar';
import { dalmatians } from '@/components/celebration/scenes/dalmatians';
import { darts } from '@/components/celebration/scenes/darts';
import { devil } from '@/components/celebration/scenes/devil';
import { galaxy } from '@/components/celebration/scenes/galaxy';
import { glitch } from '@/components/celebration/scenes/glitch';
import { hearts } from '@/components/celebration/scenes/hearts';
import { jackpot } from '@/components/celebration/scenes/jackpot';
import { kilobyte } from '@/components/celebration/scenes/kilobyte';
import { liftOff } from '@/components/celebration/scenes/lift-off';
import { marignan } from '@/components/celebration/scenes/marignan';
import { matrix } from '@/components/celebration/scenes/matrix';
import { medium } from '@/components/celebration/scenes/medium';
import { mille } from '@/components/celebration/scenes/mille';
import { night } from '@/components/celebration/scenes/night';
import { pie } from '@/components/celebration/scenes/pie';
import { pixels } from '@/components/celebration/scenes/pixels';
import { siren } from '@/components/celebration/scenes/siren';
import { small } from '@/components/celebration/scenes/small';
import { streak } from '@/components/celebration/scenes/streak';
import { tiles } from '@/components/celebration/scenes/tiles';
import { wink } from '@/components/celebration/scenes/wink';

/** Numbers with a scene of their own. */
const BESPOKE: Record<number, SceneFactory> = {
  42: galaxy,
  69: wink,
  101: dalmatians,
  112: siren,
  180: darts,
  214: hearts,
  256: pixels,
  300: bowling,
  314: pie,
  321: liftOff,
  365: calendar,
  404: glitch,
  512: pixels,
  666: devil,
  777: jackpot,
  1000: mille,
  1001: night,
  1024: kilobyte,
  1337: matrix,
  1515: marignan,
  1664: beer,
  1789: bastille,
  2000: mille,
  2048: tiles,
};

export function sceneFor(m: Milestone, t: Theme) {
  // Streaks: the generic parties (the bespoke ones are about question numbers), with Paillette hopping in.
  if (m.streak) return streak(m, t);
  const own = BESPOKE[m.n];
  if (own) return own(m, t);
  return (tier(m.n) === 'small' ? small : medium)(m, t);
}
