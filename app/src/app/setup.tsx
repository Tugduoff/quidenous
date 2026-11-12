import { LogOut } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';

import { RoomForms } from '@/components/room-forms';
import { Screen } from '@/components/screen';
import { Display, Logo } from '@/components/ui';
import { pendingJoinCode } from '@/lib/auth';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** Signed in, no room yet: join one with a code (an invite link fills it in) or create one. */
export default function Setup() {
  const t = useTheme();
  const { enterRoom, signOut } = useSession();
  const code = pendingJoinCode() ?? '';
  return (
    <Screen tabs={false}>
      <View style={{ marginTop: 32, gap: 10, marginBottom: 8 }}>
        <Logo />
        <Display size={32}>Ton salon</Display>
      </View>
      <RoomForms joinFirst={!!code} initialCode={code} onEntered={enterRoom} />
      <View style={{ flex: 1 }} />
      <Pressable
        accessibilityRole="button"
        onPress={signOut}
        hitSlop={8}
        style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
        <LogOut size={16} color={t.mut} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>Se déconnecter</Text>
      </Pressable>
    </Screen>
  );
}
