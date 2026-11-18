

/** Shown next to a name from this many days (1 is just "voted today"). */
export const SHOWN_FROM = 2;

/** From this many days the flame takes the accent colour. */
export const HOT_FROM = 7;

/** My streak gets a party at these, once per room. */
export const STREAK_MILESTONES = [7, 30, 100] as const;

/** Highest streak milestone at or below `n` (0 when none). */
export const lastStreakMilestone = (n: number) => Math.max(0, ...STREAK_MILESTONES.filter((m) => m <= n));
