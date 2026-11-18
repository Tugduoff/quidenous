import { useEffect } from 'react';

/** Once the countdown hits 0: refetch a little later (and again every few seconds), in case the closing event got lost. */
export function useRefetchAtZero(ms: number | null, refetch: () => void) {
  const over = ms === 0;
  useEffect(() => {
    if (!over) return;
    let again: ReturnType<typeof setInterval>;
    const first = setTimeout(() => {
      refetch();
      again = setInterval(refetch, 4000);
    }, 1500);
    return () => {
      clearTimeout(first);
      clearInterval(again);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [over]);
}
