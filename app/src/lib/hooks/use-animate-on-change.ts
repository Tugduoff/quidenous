import { useEffect, useEffectEvent } from 'react';
import type { SharedValue } from 'react-native-reanimated';

type Key = string | number | boolean | null | undefined;

export function useAnimateOnChange<T>(value: SharedValue<T>, key: Key | readonly Key[], next: () => T | undefined) {
  const start = useEffectEvent(() => {
    const to = next();
    if (to !== undefined) value.set(to);
  });
  const changed = Array.isArray(key) ? JSON.stringify(key) : key;
  useEffect(() => start(), [changed]);
}
