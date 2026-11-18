import { useReducer } from 'react';
import { initMatch, matchReducer } from '@/lib/utils/said';

/**
 * The game's state for these answers (ids in the order shown). The answers are frozen once the guessing starts, so the
 * order never changes under a game; a screen that could get another set keys its board by the order.
 */
export function useMatchGame(order: number[]) {
  return useReducer(matchReducer, order, (o) => initMatch(o));
}
