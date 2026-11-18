import type { Badge } from '@/lib/api/badges';

/** The next threshold to reach (null once gold). */
export const nextGoal = (b: Badge) => (b.tier < 3 ? (b.tiers[b.tier] ?? null) : null);
