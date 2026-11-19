import { useState, useEffect } from 'react';

export function useCooldown(seconds = 30): [number, () => void] {
  const [left, setLeft] = useState(0);
  useEffect(() => {
    if (!left) return;
    const id = setTimeout(() => setLeft((l) => l - 1), 1000);
    return () => clearTimeout(id);
  }, [left]);
  return [left, () => setLeft(seconds)];
}
