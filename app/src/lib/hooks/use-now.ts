import { useEffect, useState } from 'react';

export function useNow(everyMs: number | null) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (everyMs == null) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(id);
  }, [everyMs]);
  return now;
}
