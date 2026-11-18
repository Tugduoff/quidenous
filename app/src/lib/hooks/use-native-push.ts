import * as Notifications from 'expo-notifications';
import { type Href, router } from 'expo-router';
import { useEffect, useRef } from 'react';
import { Platform } from 'react-native';
import { pathOf, registerNativePush } from '@/lib/device/native-push';
import { useSwitchRoomHere } from '@/lib/hooks/use-switch-room';
import { useLatest } from '@/lib/hooks/use-latest';
import { useSession } from '@/lib/session';

export function useNotificationTaps(active: boolean) {
  const { room } = useSession();
  const switchHere = useSwitchRoomHere();
  const latest = useLatest({ room, switchHere });
  const handled = useRef(new Set<string>());

  useEffect(() => {
    if (!active || Platform.OS !== 'android') return;
    const open = async (response: Notifications.NotificationResponse) => {
      const { request, date } = response.notification;
      const key = `${request.identifier}:${date}`;
      if (handled.current.has(key)) return;
      handled.current.add(key);
      Notifications.clearLastNotificationResponse();
      const trigger = request.trigger as { remoteMessage?: { data?: Record<string, string> } } | null;
      const data: Record<string, unknown> = { ...trigger?.remoteMessage?.data, ...request.content.data };
      const target = Number(data.room) || null;
      const { room, switchHere } = latest.current;
      const ok = target == null || target === room || (await switchHere(target).catch(() => false));
      router.navigate((ok ? pathOf(data.url) : '/') as Href);
    };
    const last = Notifications.getLastNotificationResponse();
    if (last) open(last);
    const sub = Notifications.addNotificationResponseReceivedListener(open);
    return () => sub.remove();
  }, [active, latest]);
}

export function useRegisterNativePush(signedIn: boolean) {
  useEffect(() => {
    if (signedIn) registerNativePush().catch(() => {});
  }, [signedIn]);
}
