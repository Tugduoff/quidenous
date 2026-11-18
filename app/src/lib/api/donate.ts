import { useQuery } from '@tanstack/react-query';
import { Platform } from 'react-native';
import { api } from '@/lib/api/client';
import { SIDELOAD } from '@/lib/device/update';
import { useFeatures } from '@/lib/api/features';

/**
 * "Buy us a coffee" (docs/DONATE.md), only where store rules allow a link to an outside payment page: the web
 * app and the APK installed by hand. Never in the Google Play bundle (built and OTA-exported without
 * EXPO_PUBLIC_DISTRIBUTION=sideload) nor in an iOS build: their billing policies cover tips to the developer.
 */
export const DONATE_ALLOWED = Platform.OS === 'web' || SIDELOAD;

export const donateKeys = {
  url: ['donate'] as const,
};

/** The donation page when the server has one (`donate` in /api/features) and this build may show it. */
export function useDonateUrl(): string | null {
  const { donate } = useFeatures();
  const on = DONATE_ALLOWED && donate;
  const { data } = useQuery({
    queryKey: donateKeys.url,
    queryFn: () => api<{ url: string }>('/donate'),
    enabled: on,
    staleTime: Infinity,
  });
  const url = data?.url;
  return on && typeof url === 'string' && url.startsWith('https://') ? url : null;
}
