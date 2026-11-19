import * as SplashScreen from 'expo-splash-screen';
import * as SystemUI from 'expo-system-ui';
import { useEffect } from 'react';
import { Platform } from 'react-native';

export function useHideSplash(ready: boolean) {
  useEffect(() => {
    if (ready) SplashScreen.hideAsync();
  }, [ready]);
}

export function useWindowBackground(color: string) {
  useEffect(() => {
    if (Platform.OS !== 'web') SystemUI.setBackgroundColorAsync(color).catch(() => {});
  }, [color]);
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    document.documentElement.style.backgroundColor = color;
    document.body.style.backgroundColor = color;
    for (const meta of document.querySelectorAll('meta[name="theme-color"]')) meta.setAttribute('content', color);
  }, [color]);
}
