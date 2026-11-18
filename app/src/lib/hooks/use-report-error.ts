import { useEffect } from 'react';
import { reportClientError } from '@/lib/device/crash-report';

export function useReportError(error: unknown) {
  useEffect(() => {
    void reportClientError(error);
  }, [error]);
}
