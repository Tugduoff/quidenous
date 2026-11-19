import { useQuery } from '@tanstack/react-query';
import { Platform } from 'react-native';
import { api } from '@/lib/api/client';
import { SIDELOAD } from '@/lib/device/update';
import { useFeatures } from '@/lib/api/features';

const DONATE_ALLOWED = Platform.OS === 'web' || SIDELOAD;

const donateKeys = {
  url: ['donate'] as const,
};

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
