// The web has no native push (it uses Web Push, see push.ts). This stand-in keeps expo-notifications out of the
// web bundle: importing it there logs its "not supported on web" warnings on every load.
import type { PushState } from '@/lib/device/web-push';

export async function nativePushState(): Promise<PushState> {
  return 'unsupported';
}

export async function registerNativePush() {}

/** A web notification opens its own URL (`?room=`): see `plainStart` in api.ts. */
export const openedFromNotification = () => false;
