import { type RefObject, useEffect } from 'react';
import { Platform } from 'react-native';

export function useAutoGrowTextArea(input: RefObject<unknown>, text: string) {
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    const el = input.current as HTMLTextAreaElement | null;
    if (!el?.style) return;
    el.style.height = '0px';
    el.style.height = `${el.scrollHeight}px`;
  }, [input, text]);
}
