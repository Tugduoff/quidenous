import { type Milestone, tier } from '@/lib/milestones';
import type { Theme } from '@/lib/theme';

import type { SceneFactory } from '../scene';
import { medium, small } from './basic';
import { SCENES as EPIC_A } from './epic-a';
import { SCENES as EPIC_B } from './epic-b';
import { SCENES as THEMED } from './themed';

/** Numbers with a scene of their own. */
const BESPOKE: Record<number, SceneFactory> = { ...THEMED, ...EPIC_A, ...EPIC_B };

export function sceneFor(m: Milestone, t: Theme) {
  const own = BESPOKE[m.n];
  if (own) return own(m, t);
  return (tier(m.n) === 'small' ? small : medium)(m, t);
}
