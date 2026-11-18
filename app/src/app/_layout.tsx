import { DarkTheme, DefaultTheme, ThemeProvider } from 'expo-router';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import { useState } from 'react';
import { StyleSheet, View } from 'react-native';
import { useReducedMotion } from 'react-native-reanimated';
import { AppHosts } from '@/components/root/app-hosts';
import { AppProviders } from '@/components/root/app-providers';
import { AppStack } from '@/components/root/app-stack';
import { InstallBanner, InstallScreen, installMode } from '@/components/update/install';
import { installGlobalErrorHandlers } from '@/lib/device/crash-report';
import { useAppBoot } from '@/lib/hooks/use-app-boot';
import { useScheme, useTheme } from '@/lib/theme';

// A screen that crashes shows "Something went wrong / Retry" and reports the error (POST /api/client-errors).
export { ErrorBoundary } from '@/components/ui/error-boundary';

SplashScreen.preventAutoHideAsync();
// Errors outside rendering (event handlers, rejected promises) are reported too (release builds only).
installGlobalErrorHandlers();

function Root() {
  // On a computer the web app only shows how to open it on a phone; in a phone browser, a banner to install it.
  const [install] = useState(installMode);
  const [banner, setBanner] = useState(install === 'banner');
  const [steps, setSteps] = useState(false);
  const { stage, token, loaded } = useAppBoot(install === 'block');
  const t = useTheme();
  const scheme = useScheme();
  const reduced = useReducedMotion();
  if (!loaded) return null;
  if (install === 'block') return <InstallScreen />;
  // Navigation's own background (cards, tab scenes) follows the theme too, instead of its default white.
  const base = scheme === 'light' ? DefaultTheme : DarkTheme;
  const navTheme = { ...base, colors: { ...base.colors, background: t.bg, card: t.bg, border: t.line, text: t.txt } };
  return (
    <ThemeProvider value={navTheme}>
      <StatusBar style={scheme === 'light' ? 'dark' : 'light'} />
      {banner && <InstallBanner onOpen={() => setSteps(true)} onClose={() => setBanner(false)} />}
      {/* Pages push in from the right (the iOS slide, on Android too); the inbox rises like a sheet.
          Signing in or out cross-fades. Under reduced motion screens just swap. */}
      <AppStack stage={stage} reduced={reduced} />
      {steps && (
        <View style={StyleSheet.absoluteFill}>
          <InstallScreen onClose={() => setSteps(false)} />
        </View>
      )}
      <AppHosts stage={stage} signedIn={!!token} />
    </ThemeProvider>
  );
}

export default function Layout() {
  return (
    <AppProviders>
      <Root />
    </AppProviders>
  );
}
