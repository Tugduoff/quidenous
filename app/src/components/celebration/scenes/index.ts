import { type Milestone, tier } from '@/lib/utils/milestones';
import type { Theme } from '@/lib/theme';

import type { SceneFactory } from '@/components/celebration/scene';
import { medium, small } from '@/components/celebration/scenes/basic';
import { SCENES as EPIC_A } from '@/components/celebration/scenes/epic-a';
import { SCENES as EPIC_B } from '@/components/celebration/scenes/epic-b';
import { streak } from '@/components/celebration/scenes/streak';
import { SCENES as THEMED } from '@/components/celebration/scenes/themed';

/** Numbers with a scene of their own. */
const BESPOKE: Record<number, SceneFactory> = { ...THEMED, ...EPIC_A, ...EPIC_B };

export function sceneFor(m: Milestone, t: Theme) {
  // Streaks: the generic parties (the bespoke ones are about question numbers), with Paillette hopping in.
  if (m.streak) return streak(m, t);
  const own = BESPOKE[m.n];
  if (own) return own(m, t);
  return (tier(m.n) === 'small' ? small : medium)(m, t);
}
