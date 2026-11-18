import { useState, useEffect } from 'react';

/**
 * Milliseconds left until `deadline` on the server's clock, in whole seconds (rounded up, as the screen shows them),
 * 0 once past, null without a deadline. Ticks once a second, right as the second changes, so the screen re-renders 1x/s.
 */
export function useCountdown(deadline: string | null | undefined, clock: number) {
  const end = deadline ? Date.parse(deadline) : NaN;
  const [ms, setMs] = useState<number | null>(() => (Number.isFinite(end) ? wholeSeconds(end - (Date.now() + clock)) : null));
  useEffect(() => {
    if (!Number.isFinite(end)) return;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      const left = Math.max(0, end - (Date.now() + clock));
      setMs(wholeSeconds(left));
      // The next change is when `left` drops under the whole second below it.
      if (left > 0) timer = setTimeout(tick, left - (wholeSeconds(left) - 1000) + 5);
    };
    tick();
    return () => clearTimeout(timer);
  }, [end, clock]);
  return Number.isFinite(end) ? ms : null;
}

const wholeSeconds = (ms: number) => Math.max(0, Math.ceil(ms / 1000) * 1000);
