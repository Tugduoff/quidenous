import { BricolageGrotesque_800ExtraBold } from '@expo-google-fonts/bricolage-grotesque';
import { Inter_400Regular, Inter_600SemiBold, Inter_700Bold } from '@expo-google-fonts/inter';
import { QueryClientProvider } from '@tanstack/react-query';
import { useFonts } from 'expo-font';
import { DarkTheme, DefaultTheme, router, Stack, ThemeProvider, useGlobalSearchParams, usePathname } from 'expo-router';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import * as SystemUI from 'expo-system-ui';
import { Fragment, type ReactNode, useEffect, useRef, useState } from 'react';
import { Platform, StyleSheet, View } from 'react-native';
import { useReducedMotion } from 'react-native-reanimated';

import { AppLock } from '@/components/app-lock';
import { CelebrationPreview } from '@/components/celebration';
import { InstallBanner, InstallScreen, installMode } from '@/components/install';
import { TermsGate } from '@/components/terms';
import { ToastHost } from '@/components/toast';
import { ConfirmHost } from '@/components/ui';
import { OtaToast, UpdateDialog } from '@/components/update';
import { UpdateRequired } from '@/components/update-required';
import { pendingJoinCode, pendingPlayToken, setPendingCode } from '@/lib/auth';
import { presetJoinCode } from '@/lib/api';
import { useLang } from '@/lib/i18n';
import { useLive } from '@/lib/live';
import { useLock } from '@/lib/lock';
import { useOtaCheck } from '@/lib/ota';
import { queryClient } from '@/lib/query-client';
import { registerNativePush, useNotificationTaps } from '@/lib/native-push';
import { useNotifTzSync } from '@/lib/notifications';
import { useRoomParam } from '@/lib/rooms';
import { SessionProvider, useSession } from '@/lib/session';
import { useScheme, useTheme, useThemeLoaded } from '@/lib/theme';
import { useUpdateCheck } from '@/lib/update';

// A screen that crashes shows "Something went wrong / Retry" and reports the error (POST /api/client-errors).
export { ErrorBoundary } from '@/components/error-boundary';

SplashScreen.preventAutoHideAsync();

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
  useStageRoute(stage, ready && !!fonts && themeLoaded && lock.loaded && install !== 'block');
  useNotificationTaps(inApp);
  useEffect(() => {
    if (stage === 'in') registerNativePush().catch(() => {});
  }, [stage]);
  useNotifTzSync(stage === 'in');
  // The APK installed by hand looks for a newer release (never in the Play build).
  useUpdateCheck(ready && install !== 'block');
  // Over-the-air JS updates (both builds): checked again when the app comes back to the foreground.
  useOtaCheck(ready);
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
      {(stage === 'in' || stage === 'room') && <TermsGate />}
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

type Stage = 'out' | 'verify' | 'room' | 'in';
const STAGE_HOME: Record<Stage, string> = { out: '/join', verify: '/auth/verify', room: '/setup', in: '/' };
/** What a signed-in account without a room can open besides /setup: the account itself (and deleting it). */
const ROOMLESS = ['/setup', '/profile', '/friends', '/security', '/notifications'];
const SIGNED_OUT = ['/join', '/auth/email', '/auth/password', '/auth/two-factor', '/auth/forgot', '/auth/reset', '/auth/mfa-reset'];

/**
 * Keep the screen on the current stage: when signing up, in or out changes it, go to that stage's screen.
 * Without this the router falls back to the only unguarded route (/oauth), which bounced to `/` and back forever.
 */
/** The invite code the page was opened with (read once at load) has been handled. */
let presetUsed = false;

function useStageRoute(stage: Stage, mounted: boolean) {
  const pathname = usePathname();
  const { code, error, share } = useGlobalSearchParams<{ code?: string; error?: string; share?: string }>();
  // Seen signed out (or verifying): a shared question opened meanwhile is resumed once in.
  const wasOut = useRef(false);
  const invited = useRef(false);
  useEffect(() => {
    if (!mounted) return;
    if (stage === 'out' || stage === 'verify') wasOut.current = true;
    // Back from Google / Discord with an answer: that page finishes the sign-in itself.
    if (pathname === '/oauth' && (code || error)) return;
    if ((stage === 'in' || stage === 'room') && wasOut.current) {
      wasOut.current = false;
      if (pendingPlayToken() && pathname !== '/play') return void router.replace('/play');
    }
    // A party's join link and big screen work at every stage (a guest has no account yet).
    const open = pathname.startsWith('/party/join/') || pathname.startsWith('/party/screen/');
    // /play only with a shared question to show: the router also falls back to it when the guarded screens swap.
    const play = pathname === '/play' && stage !== 'verify' && !!(share || pendingPlayToken());
    // An invite link opened while signed in: the router drops /join and lands on home; from there, on to the join form with the code filled in.
    // Opened signed out (or before any room): signing in or /setup uses that code, not this.
    if (stage !== 'in') presetUsed = true;
    if (stage === 'in' && pathname === '/join' && (code || (!presetUsed && presetJoinCode()))) {
      setPendingCode(code ?? presetJoinCode());
      invited.current = true;
      presetUsed = true;
    }
    // The links shared before 0.32.1 (`/?code=`) land straight on home: once, the same way.
    if (stage === 'in' && pathname === '/' && !presetUsed && presetJoinCode()) {
      presetUsed = true;
      setPendingCode(presetJoinCode());
      invited.current = true;
    }
    if (stage === 'in' && pathname === '/' && invited.current && pendingJoinCode()) {
      invited.current = false;
      return void router.push('/rooms');
    }
    const ok =
      open || play
        ? true
        : stage === 'out'
          ? SIGNED_OUT.includes(pathname)
          : stage === 'verify'
            ? pathname === '/auth/verify'
            : stage === 'room'
              ? ROOMLESS.includes(pathname)
              : pathname !== '/oauth' && pathname !== '/setup' && pathname !== '/join' && !pathname.startsWith('/auth');
    if (!ok) router.replace(STAGE_HOME[stage] as never);
  }, [stage, pathname, mounted, code, error, share]);
}
