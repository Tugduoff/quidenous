import { type QueryClient, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

export type NotifKind = 'question' | 'result' | 'chat' | 'mention' | 'nudge' | 'added' | 'ask' | 'streak';

export type NotifPrefs = { enabled: boolean; off: NotifKind[]; quiet_start: number | null; quiet_end: number | null; tz_offset: number };

export const notificationKeys = {
  prefs: ['notifications'] as const,
};

const tzOffset = () => -new Date().getTimezoneOffset();

const getPrefs = () => api<NotifPrefs>('/notifications');
const putPrefs = (p: NotifPrefs) => api<NotifPrefs>('/notifications', { method: 'PUT', json: { ...p, tz_offset: tzOffset() } });

export const useNotifPrefs = (enabled: boolean) => useQuery({ queryKey: notificationKeys.prefs, queryFn: getPrefs, enabled });

export function useSaveNotifPrefs() {
  const qc = useQueryClient();
  return useMutation({
    mutationKey: notificationKeys.prefs,
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
    onSettled: () => {
      if (qc.isMutating({ mutationKey: notificationKeys.prefs }) === 1) qc.invalidateQueries({ queryKey: notificationKeys.prefs });
    },
  });
}

export async function syncNotifTz(qc: QueryClient, streaks: boolean) {
  const p = await qc.fetchQuery({ queryKey: notificationKeys.prefs, queryFn: getPrefs });
  if ((p.quiet_start == null && !streaks) || p.tz_offset === tzOffset()) return;
  qc.setQueryData(notificationKeys.prefs, await putPrefs(p));
}
