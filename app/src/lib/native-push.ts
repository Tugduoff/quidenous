import * as Notifications from 'expo-notifications';
import { Platform } from 'react-native';

import { api } from './api';

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
