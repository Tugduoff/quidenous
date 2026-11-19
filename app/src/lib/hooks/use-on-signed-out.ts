import { useEffect, useEffectEvent } from 'react';
import { setOnSignedOut } from '@/lib/api/client';

export function useOnSignedOut(onSignedOut: () => void) {
  const handle = useEffectEvent(onSignedOut);
  useEffect(() => {
    setOnSignedOut(() => handle());
    return () => setOnSignedOut(null);
  }, []);
}
