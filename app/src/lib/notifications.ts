import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useState } from 'react';
import { AppState, Platform } from 'react-native';

import { api } from './api';
import { useFeatures } from './auth';
import { nativePushState, registerNativePush } from './native-push';
import { enablePush, pushState, webSubscribed } from './push';

/** What a push can be about (the inbox keeps everything; these only silence pushes). */
export type NotifKind = 'question' | 'result' | 'chat' | 'mention' | 'nudge' | 'added' | 'ask';

/** Quiet hours in minutes after local midnight (both null = none; may wrap past midnight). `enabled`: every device of the account. */
export type NotifPrefs = { enabled: boolean; off: NotifKind[]; quiet_start: number | null; quiet_end: number | null; tz_offset: number };

export const notifKey = ['notifications'] as const;

/** Minutes east of UTC, as the server wants it. */
export const tzOffset = () => -new Date().getTimezoneOffset();

const getPrefs = () => api<NotifPrefs>('/notifications');
const putPrefs = (p: NotifPrefs) => api<NotifPrefs>('/notifications', { method: 'PUT', json: { ...p, tz_offset: tzOffset() } });

export const useNotifPrefs = (enabled: boolean) => useQuery({ queryKey: notifKey, queryFn: getPrefs, enabled });

/** Saves the whole set at once; the screen shows the change before the server answers. */
export function useSaveNotifPrefs() {
  const qc = useQueryClient();
  return useMutation({
    mutationKey: notifKey,
    mutationFn: putPrefs,
    onMutate: async (next: NotifPrefs) => {
      await qc.cancelQueries({ queryKey: notifKey });
      const before = qc.getQueryData<NotifPrefs>(notifKey);
      qc.setQueryData(notifKey, next);
      return { before };
    },
    onError: (_e, _next, ctx) => {
      if (ctx?.before) qc.setQueryData(notifKey, ctx.before);
    },
    // Only after the last of several quick taps, so an older answer doesn't flip a toggle back.
    onSettled: () => {
      if (qc.isMutating({ mutationKey: notifKey }) === 1) qc.invalidateQueries({ queryKey: notifKey });
    },
  });
}

/**
 * This device: 'on' = allowed and subscribed, 'off' = can still be turned on, 'denied' = blocked by the browser / phone,
 * 'unsupported' = no push here (iPhone app, a browser without push).
 */
export type DevicePush = 'unsupported' | 'off' | 'denied' | 'on';

async function devicePush(): Promise<DevicePush> {
  if (Platform.OS === 'web') {
    const s = pushState();
    if (s === 'unsupported' || s === 'denied') return s;
    return s === 'granted' && (await webSubscribed()) ? 'on' : 'off';
  }
  const s = await nativePushState();
  return s === 'granted' ? 'on' : s === 'default' ? 'off' : s;
}

/** The device's state, checked again when the app comes back (from the phone's settings). `enable` must run from a tap. */
export function useDevicePush() {
  const [state, setState] = useState<DevicePush | null>(null);
  const refresh = useCallback(() => devicePush().then(setState, () => setState('unsupported')), []);
  useEffect(() => {
    refresh();
    const sub = AppState.addEventListener('change', (s) => s === 'active' && refresh());
    return () => sub.remove();
  }, [refresh]);
  const enable = async () => {
    try {
      if (Platform.OS === 'web') await enablePush();
      else await registerNativePush();
    } finally {
      await refresh();
    }
  };
  return { state, enable };
}

/** On start: quiet hours follow the phone's clock (summer time, travel). Silent, and only when it changed. */
export function useNotifTzSync(active: boolean) {
  const qc = useQueryClient();
  const { notificationSettings } = useFeatures();
  useEffect(() => {
    if (!active || !notificationSettings) return;
    (async () => {
      const p = await qc.fetchQuery({ queryKey: notifKey, queryFn: getPrefs });
      if (p.quiet_start == null || p.tz_offset === tzOffset()) return;
      qc.setQueryData(notifKey, await putPrefs(p));
    })().catch(() => {});
  }, [active, notificationSettings, qc]);
}
