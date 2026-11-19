import { Platform } from 'react-native';

import { getPushConfig, subscribePush } from '@/lib/api/push';

export type PushState = 'unsupported' | 'default' | 'denied' | 'granted';

const supported = () =>
  Platform.OS === 'web' &&
  typeof window !== 'undefined' &&
  'serviceWorker' in navigator &&
  'PushManager' in window &&
  'Notification' in window;

export function pushState(): PushState {
  if (!supported()) return 'unsupported';
  return Notification.permission as PushState;
}

const toUint8 = (b64url: string) => {
  const b64 = b64url.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - (b64url.length % 4)) % 4);
  return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
};

const toB64Url = (buf: ArrayBuffer | null) =>
  buf ? btoa(String.fromCharCode(...new Uint8Array(buf))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '') : '';

export async function enablePush(): Promise<PushState> {
  if (!supported()) return 'unsupported';
  const permission = await Notification.requestPermission();
  if (permission !== 'granted') return permission as PushState;
  const { vapid_public_key } = await getPushConfig();
  if (!vapid_public_key) throw new Error('push_not_configured');
  const registration = await navigator.serviceWorker.register('/sw.js');
  await navigator.serviceWorker.ready;
  const subscription =
    (await registration.pushManager.getSubscription()) ??
    (await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: toUint8(vapid_public_key),
    }));
  await sendSubscription(subscription);
  return 'granted';
}

async function sendSubscription(subscription: PushSubscription) {
  await subscribePush({
    kind: 'web',
    endpoint: subscription.endpoint,
    p256dh: toB64Url(subscription.getKey('p256dh')),
    auth: toB64Url(subscription.getKey('auth')),
  });
}

export async function webSubscribed() {
  if (pushState() !== 'granted') return false;
  const registration = await navigator.serviceWorker.getRegistration();
  return !!(await registration?.pushManager.getSubscription());
}

export async function refreshPush() {
  if (pushState() !== 'granted') return;
  const registration = await navigator.serviceWorker.register('/sw.js');
  const subscription = await registration.pushManager.getSubscription();
  if (subscription) await sendSubscription(subscription);
}
