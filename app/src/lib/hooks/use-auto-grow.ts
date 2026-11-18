import { type RefObject, useEffect } from 'react';
import { Platform } from 'react-native';

function fitHeight(el: HTMLTextAreaElement | null) {
  if (!el?.style) return;
  el.style.height = '0px';
  el.style.height = `${el.scrollHeight}px`;
}

export function useAutoGrowTextArea(input: RefObject<unknown>, text: string) {
  useEffect(() => {
    if (Platform.OS === 'web') fitHeight(input.current as HTMLTextAreaElement | null);
  }, [input, text]);
}
