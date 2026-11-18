import { useState } from 'react';
import { useTimeout } from '@/lib/hooks/use-timeout';

export function useDebouncedValue<T extends string | number | boolean | null | undefined>(value: T, ms: number): T {
  const [settled, setSettled] = useState(value);
  useTimeout(() => setSettled(value), ms, value);
  return settled;
}
