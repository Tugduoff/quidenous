import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { useFeatures } from '@/lib/api/features';
import { syncNotifTz } from '@/lib/api/notifications';

export function useNotifTzSync(active: boolean) {
  const qc = useQueryClient();
  const { notificationSettings, streaks } = useFeatures();
  useEffect(() => {
    if (!active || !notificationSettings) return;
    syncNotifTz(qc, streaks).catch(() => {});
  }, [active, notificationSettings, streaks, qc]);
}
