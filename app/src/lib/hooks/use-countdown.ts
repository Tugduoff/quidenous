import { useState, useEffect } from 'react';

export function useCountdown(deadline: string | null | undefined, clock: number) {
  const end = deadline ? Date.parse(deadline) : NaN;
  const [ms, setMs] = useState<number | null>(() => (Number.isFinite(end) ? wholeSeconds(end - (Date.now() + clock)) : null));
  useEffect(() => {
    if (!Number.isFinite(end)) return;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      const left = Math.max(0, end - (Date.now() + clock));
      setMs(wholeSeconds(left));
      if (left > 0) timer = setTimeout(tick, left - (wholeSeconds(left) - 1000) + 5);
    };
    tick();
    return () => clearTimeout(timer);
  }, [end, clock]);
  return Number.isFinite(end) ? ms : null;
}

const wholeSeconds = (ms: number) => Math.max(0, Math.ceil(ms / 1000) * 1000);
