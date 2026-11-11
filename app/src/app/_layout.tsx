import { BricolageGrotesque_800ExtraBold } from '@expo-google-fonts/bricolage-grotesque';
import { Inter_400Regular, Inter_600SemiBold, Inter_700Bold } from '@expo-google-fonts/inter';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useFonts } from 'expo-font';
import { Stack } from 'expo-router';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import { useEffect } from 'react';
import { Platform } from 'react-native';

import { useLive } from '@/lib/live';
import { registerNativePush } from '@/lib/native-push';
import { SessionProvider, useSession } from '@/lib/session';
import { useScheme, useTheme } from '@/lib/theme';

SplashScreen.preventAutoHideAsync();
const queryClient = new QueryClient({ defaultOptions: { queries: { staleTime: 10_000 } } });

function Root() {
  const { ready, token } = useSession();
  const t = useTheme();
  const scheme = useScheme();
  const [fonts] = useFonts({ BricolageGrotesque_800ExtraBold, Inter_400Regular, Inter_600SemiBold, Inter_700Bold });
  useLive(token);
  useEffect(() => {
    if (token) registerNativePush().catch(() => {});
  }, [token]);
  useEffect(() => {
    if (ready && fonts) SplashScreen.hideAsync();
  }, [ready, fonts]);
  // On the web, iOS paints the area under the clock with the page background or the theme-color:
  // both follow the theme chosen in the app, not only the one of the phone.
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    document.documentElement.style.backgroundColor = t.bg;
    document.body.style.backgroundColor = t.bg;
    for (const meta of document.querySelectorAll('meta[name="theme-color"]')) meta.setAttribute('content', t.bg);
  }, [t.bg]);
  if (!ready || !fonts) return null;
  return (
    <>
      <StatusBar style={scheme === 'light' ? 'dark' : 'light'} />
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: t.bg } }}>
        <Stack.Protected guard={!!token}>
          <Stack.Screen name="(tabs)" />
          <Stack.Screen name="round/[id]" />
          <Stack.Screen name="profile" />
          <Stack.Screen name="group" />
          <Stack.Screen name="inbox" />
        </Stack.Protected>
        <Stack.Protected guard={!token}>
          <Stack.Screen name="join" />
        </Stack.Protected>
      </Stack>
    </>
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
