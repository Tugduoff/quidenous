import { useEffect, useEffectEvent, useRef } from 'react';

export function useOnChange<T>(value: T, onChange: (value: T, previous: T) => void) {
  const handle = useEffectEvent(onChange);
  const previous = useRef(value);
  useEffect(() => {
    const was = previous.current;
    if (Object.is(was, value)) return;
    previous.current = value;
    handle(value, was);
  }, [value]);
}

export function useWhen(condition: boolean, onTrue: () => void) {
  const handle = useEffectEvent(onTrue);
  useEffect(() => {
    if (condition) handle();
  }, [condition]);
}
