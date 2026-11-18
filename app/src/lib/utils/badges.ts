import type { Badge } from '@/lib/api/badges';

export const nextGoal = (b: Badge) => (b.tier < 3 ? (b.tiers[b.tier] ?? null) : null);
