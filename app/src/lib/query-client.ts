import { focusManager, MutationCache, QueryClient } from '@tanstack/react-query';
import { AppState, Platform } from 'react-native';

import { showToast } from '@/components/toast';

import { ApiError } from './api';
import { authError } from './auth';

declare module '@tanstack/react-query' {
  interface Register {
    mutationMeta: {
      /** The screen shows this error itself (all of them, or those the function picks): no toast. */
      quiet?: boolean | ((error: unknown) => boolean);
    };
  }
}

/**
 * The app's query cache. An action that fails says so in a toast, unless its screen already shows the error
 * (`meta: { quiet }`). Signed out (401) is handled by the session.
 */
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

/**
 * Forget the previous room's (or account's) data on a switch or a sign-in, but not what the server can do
 * (`features`): the live socket needs it at once to ask for a ticket instead of putting the token in its URL.
 * Nor « Aujourd'hui » (`today`): all my rooms, the same whichever is on screen (signing out clears everything).
 */
export function clearRoomData(qc: QueryClient) {
  qc.getMutationCache().clear();
  qc.removeQueries({ predicate: (q) => q.queryKey[0] !== 'features' && q.queryKey[0] !== 'today' });
}

// On a phone, coming back to the app counts as focus: what's on screen refreshes (the web does this by itself).
if (Platform.OS !== 'web')
  focusManager.setEventListener((onFocus) => {
    const sub = AppState.addEventListener('change', (state) => onFocus(state === 'active'));
    return () => sub.remove();
  });
