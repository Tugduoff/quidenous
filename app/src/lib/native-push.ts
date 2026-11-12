import * as Notifications from 'expo-notifications';
import { type Href, router } from 'expo-router';
import { useEffect, useRef } from 'react';
import { Platform } from 'react-native';

import { api } from './api';
import type { PushState } from './push';
import { useSwitchRoomHere } from './rooms';
import { useSession } from './session';

/** Android: 'denied' once the system won't ask again (only the phone's settings can turn them back on). */
export async function nativePushState(): Promise<PushState> {
  if (Platform.OS !== 'android') return 'unsupported';
  const p = await Notifications.getPermissionsAsync();
  return p.granted ? 'granted' : p.canAskAgain ? 'default' : 'denied';
}

/** Android build: FCM token → server. The server sends through Firebase (see server/src/push.rs). */
export async function registerNativePush() {
  if (Platform.OS !== 'android') return;
  await Notifications.setNotificationChannelAsync('default', {
    name: 'Questions',
    importance: Notifications.AndroidImportance.HIGH,
    lightColor: '#C8FF3D',
  });
  const current = await Notifications.getPermissionsAsync();
  const granted = current.granted || (await Notifications.requestPermissionsAsync()).granted;
  if (!granted) return;
  const { data: token } = await Notifications.getDevicePushTokenAsync();
  await api('/push/subscribe', { method: 'POST', json: { kind: 'fcm', token } });
}

/** The app path of a notification (`/round/12?room=3` → `/round/12`): the room is switched to before. */
const pathOf = (url: unknown) => {
  if (typeof url !== 'string' || !url.startsWith('/')) return '/';
  const [path, query = ''] = url.split('?');
  const params = new URLSearchParams(query);
  params.delete('room');
  const rest = params.toString();
  return rest ? `${path}?${rest}` : path;
};

/**
 * Android: a tap on a notification opens its screen, in its room, whether the app was running or not (cold start).
 * Mounted once from the root layout, `active` once signed in and in a room.
 */
export function useNotificationTaps(active: boolean) {
  const { room } = useSession();
  const switchHere = useSwitchRoomHere();
  // The listener is set once: it reads the current room and switcher from here.
  const latest = useRef({ room, switchHere });
  latest.current = { room, switchHere };
  const handled = useRef(new Set<string>());

  useEffect(() => {
    if (!active || Platform.OS !== 'android') return;
    const open = async (response: Notifications.NotificationResponse) => {
      const { request, date } = response.notification;
      const key = `${request.identifier}:${date}`;
      if (handled.current.has(key)) return;
      handled.current.add(key);
      Notifications.clearLastNotificationResponse();
      // FCM `data` (strings): in the content, or on the raw message when Android showed it itself.
      const trigger = request.trigger as { remoteMessage?: { data?: Record<string, string> } } | null;
      const data: Record<string, unknown> = { ...trigger?.remoteMessage?.data, ...request.content.data };
      const target = Number(data.room) || null;
      const { room, switchHere } = latest.current;
      // A room I'm no longer in: its screens would fail, home instead.
      const ok = target == null || target === room || (await switchHere(target).catch(() => false));
      router.navigate((ok ? pathOf(data.url) : '/') as Href);
    };
    const last = Notifications.getLastNotificationResponse();
    if (last) open(last);
    const sub = Notifications.addNotificationResponseReceivedListener(open);
    return () => sub.remove();
  }, [active]);
}
