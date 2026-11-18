import { BricolageGrotesque_800ExtraBold } from '@expo-google-fonts/bricolage-grotesque';
import { Inter_400Regular, Inter_600SemiBold, Inter_700Bold } from '@expo-google-fonts/inter';
import { useFonts } from 'expo-font';
import { useLock } from '@/lib/device/lock';
import { useLive } from '@/lib/hooks/use-live';
import { useNotificationTaps, useRegisterNativePush } from '@/lib/hooks/use-native-push';
import { useNotifTzSync } from '@/lib/hooks/use-notif-tz-sync';
import { useHideSplash, useWindowBackground } from '@/lib/hooks/use-root-chrome';
import { type Stage, useStageRoute } from '@/lib/hooks/use-stage-route';
import { useRoomParam } from '@/lib/hooks/use-switch-room';
import { useOtaCheck, useUpdateCheck } from '@/lib/hooks/use-update-check';
import { useWidgetSync } from '@/lib/hooks/use-widget-sync';
import { useSession } from '@/lib/session';
import { useTheme, useThemeLoaded } from '@/lib/theme';

export function useAppBoot(installBlocked: boolean) {
  const { ready, token, room, roomless, verifying, intro } = useSession();
  // Signed out → sign-in screens; just signed up → email code, then the Petons' intro; no room yet → join or create one; else the app.
  const stage: Stage = !token ? 'out' : verifying ? 'verify' : intro ? 'intro' : roomless ? 'room' : 'in';
  const lock = useLock();
  const t = useTheme();
  const themeLoaded = useThemeLoaded();
  const [fonts] = useFonts({ BricolageGrotesque_800ExtraBold, Inter_400Regular, Inter_600SemiBold, Inter_700Bold });
  useLive(installBlocked || stage !== 'in' ? null : token, room);
  // A tapped notification opens its screen in its own room (`?room=` on the web, the tap itself on Android).
  // Only once the screens below are mounted: navigating before that fails.
  const inApp = ready && !!fonts && themeLoaded && lock.loaded && !installBlocked && stage === 'in';
  useRoomParam(inApp);
  useStageRoute(stage, ready && !!fonts && themeLoaded && lock.loaded && !installBlocked);
  useNotificationTaps(inApp);
  // The home-screen widget (Android) follows the session, the room and what happened while the app was open.
  useWidgetSync(ready ? stage : null, room);
  useRegisterNativePush(stage === 'in');
  useNotifTzSync(stage === 'in');
  // The APK installed by hand looks for a newer release (never in the Play build).
  useUpdateCheck(ready && !installBlocked);
  // Over-the-air JS updates (both builds): checked again when the app comes back to the foreground.
  useOtaCheck(ready);
  useHideSplash(!!(ready && fonts && themeLoaded && lock.loaded));
  useWindowBackground(t.bg);
  return { stage, token, loaded: !!(ready && fonts && themeLoaded && lock.loaded) };
}
