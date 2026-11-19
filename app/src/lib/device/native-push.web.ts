import type { PushState } from '@/lib/device/web-push';

export async function nativePushState(): Promise<PushState> {
  return 'unsupported';
}

export async function registerNativePush() {}

export const openedFromNotification = () => false;
