import { BricolageGrotesque_800ExtraBold } from '@expo-google-fonts/bricolage-grotesque';
import { Inter_400Regular, Inter_600SemiBold, Inter_700Bold } from '@expo-google-fonts/inter';
import { QueryClientProvider } from '@tanstack/react-query';
import { useFonts } from 'expo-font';
import { DarkTheme, DefaultTheme, Stack, ThemeProvider } from 'expo-router';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import { Fragment, type ReactNode, useState } from 'react';
import { StyleSheet, View } from 'react-native';
import { useReducedMotion } from 'react-native-reanimated';
import { AppLock } from '@/components/security/app-lock';
import { CelebrationPreview } from '@/components/celebration';
import { InstallBanner, InstallScreen, installMode } from '@/components/update/install';
import { TermsGate } from '@/components/auth/terms';
import { ToastHost } from '@/components/ui/toast';
import { OtaToast, UpdateDialog } from '@/components/update/update';
import { UpdateRequired } from '@/components/update/update-required';
import { installGlobalErrorHandlers } from '@/lib/device/crash-report';
import { useLang } from '@/lib/i18n';
import { useLive } from '@/lib/hooks/use-live';
import { useLock } from '@/lib/device/lock';
import { queryClient } from '@/lib/api/query-client';
import { useNotificationTaps, useRegisterNativePush } from '@/lib/hooks/use-native-push';
import { useNotifTzSync } from '@/lib/hooks/use-notif-tz-sync';
import { useRoomParam } from '@/lib/hooks/use-switch-room';
import { useWidgetSync } from '@/lib/hooks/use-widget-sync';
import { SessionProvider, useSession } from '@/lib/session';
import { useScheme, useTheme, useThemeLoaded } from '@/lib/theme';
import { useOtaCheck, useUpdateCheck } from '@/lib/hooks/use-update-check';
import { ConfirmHost } from '@/components/ui/confirm';
import { useStageRoute } from '@/lib/hooks/use-stage-route';
import { useHideSplash, useWindowBackground } from '@/lib/hooks/use-root-chrome';

// A screen that crashes shows "Something went wrong / Retry" and reports the error (POST /api/client-errors).
export { ErrorBoundary } from '@/components/ui/error-boundary';

SplashScreen.preventAutoHideAsync();
// Errors outside rendering (event handlers, rejected promises) are reported too (release builds only).
installGlobalErrorHandlers();

function Root() {
  const { ready, token, room, roomless, verifying, intro } = useSession();
  // Signed out → sign-in screens; just signed up → email code, then the Petons' intro; no room yet → join or create one; else the app.
  const stage = !token ? 'out' : verifying ? 'verify' : intro ? 'intro' : roomless ? 'room' : 'in';
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
  useStageRoute(stage, ready && !!fonts && themeLoaded && lock.loaded && install !== 'block');
  useNotificationTaps(inApp);
  // The home-screen widget (Android) follows the session, the room and what happened while the app was open.
  useWidgetSync(ready ? stage : null, room);
  useRegisterNativePush(stage === 'in');
  useNotifTzSync(stage === 'in');
  // The APK installed by hand looks for a newer release (never in the Play build).
  useUpdateCheck(ready && install !== 'block');
  // Over-the-air JS updates (both builds): checked again when the app comes back to the foreground.
  useOtaCheck(ready);
  useHideSplash(!!(ready && fonts && themeLoaded && lock.loaded));
  useWindowBackground(t.bg);
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
      <Stack
        screenOptions={{ headerShown: false, contentStyle: { backgroundColor: t.bg }, animation: reduced ? 'none' : 'ios_from_right' }}
        screenLayout={({ children }) => <SameLanguage>{children}</SameLanguage>}>
        <Stack.Protected guard={stage === 'in'}>
          <Stack.Screen name="(tabs)" options={{ animation: reduced ? 'none' : 'fade' }} />
          <Stack.Screen name="round/[id]" />
          <Stack.Screen name="avatar" />
          <Stack.Screen name="group" />
          <Stack.Screen name="room-settings" />
          <Stack.Screen name="inbox" options={{ animation: reduced ? 'none' : 'fade_from_bottom' }} />
          <Stack.Screen name="rooms" />
          <Stack.Screen name="packs" />
          <Stack.Screen name="pack/[id]" />
          <Stack.Screen name="pack/edit" />
          <Stack.Screen name="packs-mine" />
          <Stack.Screen name="creator/[id]" />
          <Stack.Screen name="admin" />
          <Stack.Screen name="suggest" />
          <Stack.Screen name="party/new" />
          <Stack.Screen name="party/[id]" />
        </Stack.Protected>
        <Stack.Protected guard={stage === 'room'}>
          <Stack.Screen name="setup" />
        </Stack.Protected>
        {/* The account itself (delete it included), with or without a room. */}
        <Stack.Protected guard={stage === 'in' || stage === 'room'}>
          <Stack.Screen name="profile" />
          <Stack.Screen name="friends" />
          <Stack.Screen name="security" />
          <Stack.Screen name="notifications" />
          <Stack.Screen name="badges" />
          <Stack.Screen name="wrapped" options={{ animation: reduced ? 'none' : 'fade_from_bottom' }} />
        </Stack.Protected>
        <Stack.Protected guard={stage === 'verify'}>
          <Stack.Screen name="auth/verify" />
        </Stack.Protected>
        <Stack.Protected guard={stage === 'intro'}>
          <Stack.Screen name="intro" options={{ animation: reduced ? 'none' : 'fade' }} />
        </Stack.Protected>
        <Stack.Protected guard={stage === 'out'}>
          <Stack.Screen name="join" options={{ animation: reduced ? 'none' : 'fade' }} />
          <Stack.Screen name="auth/email" />
          <Stack.Screen name="auth/password" />
          <Stack.Screen name="auth/two-factor" />
          <Stack.Screen name="auth/forgot" />
          <Stack.Screen name="auth/reset" />
          <Stack.Screen name="auth/mfa-reset" />
        </Stack.Protected>
        {/* Back from Google / Discord: signing in (signed out) or linking (signed in). */}
        <Stack.Screen name="oauth" />
        {/* A shared question to play (`/play?share=`): shown signed out too, kept through sign-in. */}
        <Stack.Screen name="play" />
        {/* A party's join link and its big-screen view: open to anyone, signed in or not. */}
        <Stack.Screen name="party/join/[code]" />
        <Stack.Screen name="party/screen/[code]" />
      </Stack>
      {steps && (
        <View style={StyleSheet.absoluteFill}>
          <InstallScreen onClose={() => setSteps(false)} />
        </View>
      )}
      <CelebrationPreview />
      <AppLock active={!!token} />
      {(stage === 'in' || stage === 'room' || stage === 'intro') && <TermsGate />}
      <ConfirmHost />
      <UpdateDialog />
      {stage === 'in' && <OtaToast />}
      <UpdateRequired />
      <ToastHost />
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

/**
 * Each screen, mounted afresh when the language changes: text that helpers (names, dates, errors) translated
 * outside `useT` is memoised by the React Compiler and would otherwise stay in the old language until a reload.
 */
function SameLanguage({ children }: { children: ReactNode }) {
  const { lang } = useLang();
  return <Fragment key={lang}>{children}</Fragment>;
}
