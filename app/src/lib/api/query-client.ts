import { focusManager, MutationCache, QueryClient } from '@tanstack/react-query';
import { AppState, Platform } from 'react-native';
import { showToast } from '@/lib/toast';
import { ApiError } from '@/lib/api/client';
import { authError } from '@/lib/api/errors';
import { featureKeys } from '@/lib/api/features';
import { todayKeys } from '@/lib/api/today';
import { seasonKeys } from '@/lib/api/season';

declare module '@tanstack/react-query' {
  interface Register {
    mutationMeta: {
      quiet?: boolean | ((error: unknown) => boolean);
    };
  }
}

export const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 10_000 } },
  mutationCache: new MutationCache({
    onError: (error, _vars, _ctx, mutation) => {
      const quiet = mutation.meta?.quiet;
      if (quiet === true || (typeof quiet === 'function' && quiet(error))) return;
      if (error instanceof ApiError && error.status === 401) return;
      showToast(authError(error));
    },
  }),
});

const KEPT_ACROSS_ROOMS = new Set<unknown>([featureKeys.all[0], todayKeys.all[0], seasonKeys.all[0]]);

export function clearRoomData(qc: QueryClient) {
  qc.getMutationCache().clear();
  qc.removeQueries({ predicate: (q) => !KEPT_ACROSS_ROOMS.has(q.queryKey[0]) });
}

if (Platform.OS !== 'web')
  focusManager.setEventListener((onFocus) => {
    const sub = AppState.addEventListener('change', (state) => onFocus(state === 'active'));
    return () => sub.remove();
  });
