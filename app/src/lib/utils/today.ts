import type { TodayRoom } from '@/lib/api/today';

/** Something only I can do: vote, or see a result. */
export const needsMe = (r: TodayRoom) => r.state === 'vote' || r.state === 'result';

/** How many votes the question still waits for. */
export const missing = (r: TodayRoom) => Math.max(0, r.needed - r.votes);

/** My streak would break tonight: it counts days and today has no vote yet, while a question is open for me. */
export const streakAtRisk = (r: TodayRoom) => r.state === 'vote' && !!r.streak && r.streak.days > 0 && !r.streak.today;

/** The question a card shows: the result to see, else the open one. */
export const cardQuestion = (r: TodayRoom) => (r.state === 'result' ? r.result : r.round);
