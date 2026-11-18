import { useReducer } from 'react';
import { initMatch, matchReducer } from '@/lib/utils/said';

export function useMatchGame(order: number[]) {
  return useReducer(matchReducer, order, (o) => initMatch(o));
}
