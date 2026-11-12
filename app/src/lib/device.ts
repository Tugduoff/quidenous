import * as Device from 'expo-device';
import { Platform } from 'react-native';

function browser(ua: string) {
  if (/EdgA?\//.test(ua)) return 'Edge';
  if (/OPR\//.test(ua)) return 'Opera';
  if (/SamsungBrowser\//.test(ua)) return 'Samsung Internet';
  if (/Firefox\/|FxiOS\//.test(ua)) return 'Firefox';
  if (/Chrome\/|CriOS\//.test(ua)) return 'Chrome';
  if (/Safari\//.test(ua)) return 'Safari';
  return 'Navigateur';
}

function system(ua: string) {
  if (/iPhone/.test(ua)) return 'iPhone';
  if (/iPad/.test(ua)) return 'iPad';
  if (/Android/.test(ua)) return 'Android';
  if (/Mac OS X/.test(ua)) return 'Mac';
  if (/Windows/.test(ua)) return 'Windows';
  if (/Linux/.test(ua)) return 'Linux';
  return null;
}

/** What this device is called in the sessions list: "iPhone 15 · iOS 18", "Android · Pixel 8", "Web · Safari (iPhone)". */
export function deviceLabel(): string {
  if (Platform.OS === 'web') {
    const ua = typeof navigator !== 'undefined' ? navigator.userAgent : '';
    const os = system(ua);
    return `Web · ${browser(ua)}${os ? ` (${os})` : ''}`;
  }
  const major = Device.osVersion?.split('.')[0];
  if (Platform.OS === 'ios') return `${Device.modelName ?? 'iPhone'} · iOS${major ? ` ${major}` : ''}`;
  return `Android${major ? ` ${major}` : ''} · ${Device.modelName ?? Device.brand ?? 'téléphone'}`;
}

/**
 * The `X-Device` header: percent-encoded, since a header value must stay ASCII
 * (Android's HTTP stack refuses "·", and the server would not read it either).
 */
export const deviceHeader = () => ({ 'x-device': encodeURIComponent(deviceLabel()) });
