import { LogOut } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { RoomForms } from '@/components/room-forms';
import { Screen } from '@/components/screen';
import { Display, Logo } from '@/components/ui';
import { pendingJoinCode } from '@/lib/auth';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** Signed in, no room yet: join one with a code (an invite link fills it in) or create one. */
export default function Setup() {
  const t = useTheme();
  const tr = useT();
  const { enterRoom, signOut } = useSession();
  const code = pendingJoinCode() ?? '';
  const [creating, setCreating] = useState(false);
  return (
    <Screen tabs={false}>
      <View style={{ marginTop: 32, gap: 10, marginBottom: 8 }}>
        <Logo />
        {!creating && <Display size={32}>{tr('auth.setup.title')}</Display>}
      </View>
      <RoomForms joinFirst={!!code} initialCode={code} onEntered={enterRoom} onVibeStep={setCreating} />
      <View style={{ flex: 1 }} />
      <Pressable
        accessibilityRole="button"
        onPress={signOut}
        hitSlop={8}
        style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
        <LogOut size={16} color={t.mut} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('auth.setup.signOut')}</Text>
      </Pressable>
    </Screen>
  );
}
