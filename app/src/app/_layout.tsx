import { BricolageGrotesque_800ExtraBold } from '@expo-google-fonts/bricolage-grotesque';
import { Inter_400Regular, Inter_600SemiBold, Inter_700Bold } from '@expo-google-fonts/inter';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useFonts } from 'expo-font';
import { DarkTheme, DefaultTheme, Stack, ThemeProvider } from 'expo-router';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import * as SystemUI from 'expo-system-ui';
import { useEffect, useState } from 'react';
import { Platform, StyleSheet, View } from 'react-native';
import { useReducedMotion } from 'react-native-reanimated';

import { AppLock } from '@/components/app-lock';
import { InstallBanner, InstallScreen, installMode } from '@/components/install';
import { ConfirmHost } from '@/components/ui';
import { useLive } from '@/lib/live';
import { useLock } from '@/lib/lock';
import { registerNativePush, useNotificationTaps } from '@/lib/native-push';
import { useRoomParam } from '@/lib/rooms';
import { SessionProvider, useSession } from '@/lib/session';
import { useScheme, useTheme, useThemeLoaded } from '@/lib/theme';

SplashScreen.preventAutoHideAsync();
const queryClient = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000 } } });

function Root() {
  const { ready, token, room, roomless, verifying } = useSession();
  // Signed out → sign-in screens; just signed up → email code; no room yet → join or create one; else the app.
  const stage = !token ? 'out' : verifying ? 'verify' : roomless ? 'room' : 'in';
  const lock = useLock();
  const t = useTheme();
  const scheme = useScheme();
  const themeLoaded = useThemeLoaded();
  const reduced = useReducedMotion();
  const [fonts] = useFonts({ BricolageGrotesque_800ExtraBold, Inter_400Regular, Inter_600SemiBold, Inter_700Bold });
  // On a computer the web app only shows how to open it on a phone; in a phone browser, a banner to install it.
  const [install] = useState(installMode);
  const [banner, setBanner] = useState(install === 'banner');
  const [steps, setSteps] = useState(false);
  useLive(install === 'block' || stage !== 'in' ? null : token, room);
  // A tapped notification opens its screen in its own room (`?room=` on the web, the tap itself on Android).
  // Only once the screens below are mounted: navigating before that fails.
  const inApp = ready && !!fonts && themeLoaded && lock.loaded && install !== 'block' && stage === 'in';
  useRoomParam(inApp);
  useNotificationTaps(inApp);
  useEffect(() => {
    if (stage === 'in') registerNativePush().catch(() => {});
  }, [stage]);
  useEffect(() => {
    if (ready && fonts && themeLoaded && lock.loaded) SplashScreen.hideAsync();
  }, [ready, fonts, themeLoaded, lock.loaded]);
  // Native: the window behind every screen (seen while one loads or slides in) takes the theme's background.
  useEffect(() => {
    if (Platform.OS !== 'web') SystemUI.setBackgroundColorAsync(t.bg).catch(() => {});
  }, [t.bg]);
  // On the web, iOS paints the area under the clock with the page background or the theme-color:
  // both follow the theme chosen in the app, not only the one of the phone.
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    document.documentElement.style.backgroundColor = t.bg;
    document.body.style.backgroundColor = t.bg;
    for (const meta of document.querySelectorAll('meta[name="theme-color"]')) meta.setAttribute('content', t.bg);
  }, [t.bg]);
  if (!ready || !fonts || !themeLoaded || !lock.loaded) return null;
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
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: t.bg }, animation: reduced ? 'none' : 'ios_from_right' }}>
        <Stack.Protected guard={stage === 'in'}>
          <Stack.Screen name="(tabs)" options={{ animation: reduced ? 'none' : 'fade' }} />
          <Stack.Screen name="round/[id]" />
          <Stack.Screen name="profile" />
          <Stack.Screen name="group" />
          <Stack.Screen name="inbox" options={{ animation: reduced ? 'none' : 'fade_from_bottom' }} />
          <Stack.Screen name="rooms" />
          <Stack.Screen name="friends" />
          <Stack.Screen name="packs" />
          <Stack.Screen name="pack/[id]" />
          <Stack.Screen name="security" />
        </Stack.Protected>
        <Stack.Protected guard={stage === 'room'}>
          <Stack.Screen name="setup" />
        </Stack.Protected>
        <Stack.Protected guard={stage === 'verify'}>
          <Stack.Screen name="auth/verify" />
        </Stack.Protected>
        <Stack.Protected guard={stage === 'out'}>
          <Stack.Screen name="join" options={{ animation: reduced ? 'none' : 'fade' }} />
          <Stack.Screen name="auth/email" />
          <Stack.Screen name="auth/password" />
          <Stack.Screen name="auth/two-factor" />
          <Stack.Screen name="auth/forgot" />
          <Stack.Screen name="auth/reset" />
        </Stack.Protected>
        {/* Back from Google / Discord: signing in (signed out) or linking (signed in). */}
        <Stack.Screen name="oauth" />
      </Stack>
      {steps && (
        <View style={StyleSheet.absoluteFill}>
          <InstallScreen onClose={() => setSteps(false)} />
        </View>
      )}
      <AppLock active={!!token} />
      <ConfirmHost />
    </ThemeProvider>
  );
}

export default function Layout() {
  return (
    <QueryClientProvider client={queryClient}>
      <SessionProvider>
        <Root />
      </SessionProvider>
    </QueryClientProvider>
  );
}
