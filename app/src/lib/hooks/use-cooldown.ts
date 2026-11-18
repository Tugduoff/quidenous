import { useState, useEffect } from 'react';

/** "Renvoyer" waits a bit between two emails: seconds left, and a way to start the wait. */
export function useCooldown(seconds = 30): [number, () => void] {
  const [left, setLeft] = useState(0);
  useEffect(() => {
    if (!left) return;
    const id = setTimeout(() => setLeft((l) => l - 1), 1000);
    return () => clearTimeout(id);
  }, [left]);
  return [left, () => setLeft(seconds)];
}
