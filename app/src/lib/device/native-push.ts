import * as Notifications from 'expo-notifications';
import { Platform } from 'react-native';

import type { PushState } from '@/lib/device/web-push';
import { subscribePush } from '@/lib/api/push';
import { ACCENTS } from '@/lib/tokens';

export async function nativePushState(): Promise<PushState> {
  if (Platform.OS !== 'android') return 'unsupported';
  const p = await Notifications.getPermissionsAsync();
  return p.granted ? 'granted' : p.canAskAgain ? 'default' : 'denied';
}

export async function registerNativePush() {
  if (Platform.OS !== 'android') return;
  await Notifications.setNotificationChannelAsync('default', {
    name: 'Questions',
    // eslint-disable-next-line import/namespace
    importance: Notifications.AndroidImportance.HIGH,
    lightColor: ACCENTS.lime.acc,
  });
  const current = await Notifications.getPermissionsAsync();
  const granted = current.granted || (await Notifications.requestPermissionsAsync()).granted;
  if (!granted) return;
  const { data: token } = await Notifications.getDevicePushTokenAsync();
  await subscribePush({ kind: 'fcm', token });
}

export const openedFromNotification = () => Platform.OS === 'android' && !!Notifications.getLastNotificationResponse();

export const pathOf = (url: unknown) => {
  if (typeof url !== 'string' || !url.startsWith('/')) return '/';
  const [path, query = ''] = url.split('?');
  const params = new URLSearchParams(query);
  params.delete('room');
  const rest = params.toString();
  return rest ? `${path}?${rest}` : path;
};
