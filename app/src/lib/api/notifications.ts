import { type QueryClient, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

/** What a push can be about (the inbox keeps everything; these only silence pushes). */
export type NotifKind = 'question' | 'result' | 'chat' | 'mention' | 'nudge' | 'added' | 'ask' | 'streak';

/** Quiet hours in minutes after local midnight (both null = none; may wrap past midnight). `enabled`: every device of the account. */
export type NotifPrefs = { enabled: boolean; off: NotifKind[]; quiet_start: number | null; quiet_end: number | null; tz_offset: number };

export const notificationKeys = {
  prefs: ['notifications'] as const,
};

/** Minutes east of UTC, as the server wants it. */
const tzOffset = () => -new Date().getTimezoneOffset();

const getPrefs = () => api<NotifPrefs>('/notifications');
const putPrefs = (p: NotifPrefs) => api<NotifPrefs>('/notifications', { method: 'PUT', json: { ...p, tz_offset: tzOffset() } });

export const useNotifPrefs = (enabled: boolean) => useQuery({ queryKey: notificationKeys.prefs, queryFn: getPrefs, enabled });

/** Saves the whole set at once; the screen shows the change before the server answers. */
export function useSaveNotifPrefs() {
  const qc = useQueryClient();
  return useMutation({
    mutationKey: notificationKeys.prefs,
    // The screen says "not saved" itself.
    meta: { quiet: true },
    mutationFn: putPrefs,
    onMutate: async (next: NotifPrefs) => {
      await qc.cancelQueries({ queryKey: notificationKeys.prefs });
      const before = qc.getQueryData<NotifPrefs>(notificationKeys.prefs);
      qc.setQueryData(notificationKeys.prefs, next);
      return { before };
    },
    onError: (_e, _next, ctx) => {
      if (ctx?.before) qc.setQueryData(notificationKeys.prefs, ctx.before);
    },
    // Only after the last of several quick taps, so an older answer doesn't flip a toggle back.
    onSettled: () => {
      if (qc.isMutating({ mutationKey: notificationKeys.prefs }) === 1) qc.invalidateQueries({ queryKey: notificationKeys.prefs });
    },
  });
}

/**
 * On start: quiet hours (and, on a server with streaks, the days they count) follow the phone's clock (summer time, travel).
 * Silent, and only when it changed.
 */
export async function syncNotifTz(qc: QueryClient, streaks: boolean) {
  const p = await qc.fetchQuery({ queryKey: notificationKeys.prefs, queryFn: getPrefs });
  if ((p.quiet_start == null && !streaks) || p.tz_offset === tzOffset()) return;
  qc.setQueryData(notificationKeys.prefs, await putPrefs(p));
}
