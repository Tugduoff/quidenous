import { Pencil } from 'lucide-react-native';
import { ActivityIndicator, Pressable, View } from 'react-native';
import { MyStreak } from '@/components/streaks/streaks';
import { MyTitle } from '@/components/badges/title-chip';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { Display } from '@/components/ui/text';
import type { Member } from '@/lib/api/group';

export function ProfileHeader({ me, accountName, photoBusy, onPhoto }: { me: Member | null; accountName: string | undefined; photoBusy: boolean; onPhoto: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <>
    {!me && accountName != null && (
      <View style={{ alignItems: 'center', marginVertical: 8 }}>
        <Display size={accountName.length > 14 ? 22 : 28}>{accountName}</Display>
      </View>
    )}
    {me && (
      <View style={{ alignItems: 'center', gap: 12, marginVertical: 8 }}>
        <Pressable accessibilityRole="button" accessibilityLabel={tr('profile.photo.change')} onPress={onPhoto}>
          <Avatar member={me} size={112} />
          <View
            style={{
              position: 'absolute',
              right: 0,
              bottom: 0,
              width: 36,
              height: 36,
              borderRadius: 18,
              backgroundColor: t.acc,
              alignItems: 'center',
              justifyContent: 'center',
              borderWidth: 3,
              borderColor: t.bg,
            }}>
            {photoBusy ? <ActivityIndicator color={t.accTxt} /> : <Pencil size={16} color={t.accTxt} />}
          </View>
        </Pressable>
        <Display size={me.name.length > 14 ? 22 : 28}>{me.name}</Display>
        <MyTitle />
        <MyStreak />
      </View>
    )}
    </>
  );
}
