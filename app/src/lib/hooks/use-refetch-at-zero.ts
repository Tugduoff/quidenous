import { useEffect, useEffectEvent } from 'react';

export function useRefetchAtZero(ms: number | null, refetch: () => void) {
  const over = ms === 0;
  const refetchNow = useEffectEvent(() => refetch());
  useEffect(() => {
    if (!over) return;
    let again: ReturnType<typeof setInterval>;
    const first = setTimeout(() => {
      refetchNow();
      again = setInterval(refetchNow, 4000);
    }, 1500);
    return () => {
      clearTimeout(first);
      clearInterval(again);
    };
  }, [over]);
}
