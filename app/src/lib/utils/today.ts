import type { TodayRoom } from '@/lib/api/today';

export const needsMe = (r: TodayRoom) => r.state === 'vote' || r.state === 'result';

export const missing = (r: TodayRoom) => Math.max(0, r.needed - r.votes);

export const streakAtRisk = (r: TodayRoom) => r.state === 'vote' && !!r.streak && r.streak.days > 0 && !r.streak.today;

export const cardQuestion = (r: TodayRoom) => (r.state === 'result' ? r.result : r.round);
