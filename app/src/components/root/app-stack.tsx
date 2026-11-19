import { Stack } from 'expo-router';
import { SameLanguage } from '@/components/root/same-language';
import { RoomThemeScope } from '@/components/themes/room-theme-scope';
import type { Stage } from '@/lib/hooks/use-stage-route';
import { useTheme } from '@/lib/theme';

const ROOM_SCREENS = new Set(['(tabs)', 'round/[id]', 'group', 'room-settings']);

export function AppStack({ stage, reduced }: { stage: Stage; reduced: boolean }) {
  const t = useTheme();
  return (
    <Stack
      screenOptions={{ headerShown: false, contentStyle: { backgroundColor: t.bg }, animation: reduced ? 'none' : 'ios_from_right' }}
      screenLayout={({ children, route }) => (
        <SameLanguage>
          <RoomThemeScope room={ROOM_SCREENS.has(route.name)}>{children}</RoomThemeScope>
        </SameLanguage>
      )}>
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
      <Stack.Screen name="oauth" />
      <Stack.Screen name="play" />
      <Stack.Screen name="party/join/[code]" />
      <Stack.Screen name="party/screen/[code]" />
    </Stack>
  );
}
