import { api } from '@/lib/api/client';

export type PushSubscriptionBody =
  | { kind: 'web'; endpoint: string; p256dh: string; auth: string }
  | { kind: 'fcm'; token: string };

export const getPushConfig = () => api<{ vapid_public_key: string | null }>('/push/config');

export const subscribePush = (subscription: PushSubscriptionBody) => api('/push/subscribe', { method: 'POST', json: subscription });
