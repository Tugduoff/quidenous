export const SHOWN_FROM = 2;

export const HOT_FROM = 7;

export const STREAK_MILESTONES = [7, 30, 100] as const;

export const lastStreakMilestone = (n: number) => Math.max(0, ...STREAK_MILESTONES.filter((m) => m <= n));
