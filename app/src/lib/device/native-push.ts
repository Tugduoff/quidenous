import * as Notifications from 'expo-notifications';
import { Platform } from 'react-native';

import type { PushState } from '@/lib/device/web-push';
import { subscribePush } from '@/lib/api/push';

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
    // eslint-disable-next-line import/namespace -- Android only, missing from the web typings
    importance: Notifications.AndroidImportance.HIGH,
    lightColor: '#C8FF3D',
  });
  const current = await Notifications.getPermissionsAsync();
  const granted = current.granted || (await Notifications.requestPermissionsAsync()).granted;
  if (!granted) return;
  const { data: token } = await Notifications.getDevicePushTokenAsync();
  await subscribePush({ kind: 'fcm', token });
}

/** Android: the app was opened (or brought back) by a tap on a notification not handled yet: it opens that screen. */
export const openedFromNotification = () => Platform.OS === 'android' && !!Notifications.getLastNotificationResponse();

/** The app path of a notification (`/round/12?room=3` → `/round/12`): the room is switched to before. */
export const pathOf = (url: unknown) => {
  if (typeof url !== 'string' || !url.startsWith('/')) return '/';
  const [path, query = ''] = url.split('?');
  const params = new URLSearchParams(query);
  params.delete('room');
  const rest = params.toString();
  return rest ? `${path}?${rest}` : path;
};

