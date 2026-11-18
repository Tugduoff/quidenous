import { useEffect } from 'react';
import { checkForUpdate, cleanUp, SIDELOAD, UPDATES } from '@/lib/device/update';
import { checkOta, OTA } from '@/lib/device/ota';
import { useAppStateChange } from '@/lib/hooks/use-app-state';

export function useUpdateCheck(active: boolean) {
  const on = active && UPDATES;
  useEffect(() => {
    if (!on) return;
    if (SIDELOAD) cleanUp();
    checkForUpdate();
  }, [on]);
  useAppStateChange((s) => s === 'active' && checkForUpdate(), on);
}

export function useOtaCheck(active: boolean) {
  useAppStateChange((s) => s === 'active' && checkOta(), active && OTA);
}
