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

import { InstallBanner, InstallScreen, installMode } from '@/components/install';
import { ConfirmHost } from '@/components/ui';
import { useLive } from '@/lib/live';
import { registerNativePush } from '@/lib/native-push';
import { SessionProvider, useSession } from '@/lib/session';
import { useScheme, useTheme, useThemeLoaded } from '@/lib/theme';

SplashScreen.preventAutoHideAsync();
const queryClient = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000 } } });

function Root() {
  const { ready, token, room } = useSession();
  const t = useTheme();
  const scheme = useScheme();
  const themeLoaded = useThemeLoaded();
  const [fonts] = useFonts({ BricolageGrotesque_800ExtraBold, Inter_400Regular, Inter_600SemiBold, Inter_700Bold });
  // On a computer the web app only shows how to open it on a phone; in a phone browser, a banner to install it.
  const [install] = useState(installMode);
  const [banner, setBanner] = useState(install === 'banner');
  const [steps, setSteps] = useState(false);
  useLive(install === 'block' ? null : token, room);
  useEffect(() => {
    if (token) registerNativePush().catch(() => {});
  }, [token]);
  useEffect(() => {
    if (ready && fonts && themeLoaded) SplashScreen.hideAsync();
  }, [ready, fonts, themeLoaded]);
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
  if (!ready || !fonts || !themeLoaded) return null;
  if (install === 'block') return <InstallScreen />;
  // Navigation's own background (cards, tab scenes) follows the theme too, instead of its default white.
  const base = scheme === 'light' ? DefaultTheme : DarkTheme;
  const navTheme = { ...base, colors: { ...base.colors, background: t.bg, card: t.bg, border: t.line, text: t.txt } };
  return (
    <ThemeProvider value={navTheme}>
      <StatusBar style={scheme === 'light' ? 'dark' : 'light'} />
      {banner && <InstallBanner onOpen={() => setSteps(true)} onClose={() => setBanner(false)} />}
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: t.bg } }}>
        <Stack.Protected guard={!!token}>
          <Stack.Screen name="(tabs)" />
          <Stack.Screen name="round/[id]" />
          <Stack.Screen name="profile" />
          <Stack.Screen name="group" />
          <Stack.Screen name="inbox" />
          <Stack.Screen name="rooms" />
          <Stack.Screen name="friends" />
          <Stack.Screen name="packs" />
          <Stack.Screen name="pack/[id]" />
        </Stack.Protected>
        <Stack.Protected guard={!token}>
          <Stack.Screen name="join" />
        </Stack.Protected>
      </Stack>
      {steps && (
        <View style={StyleSheet.absoluteFill}>
          <InstallScreen onClose={() => setSteps(false)} />
        </View>
      )}
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
