import { router } from 'expo-router';
import { Pressable, Text, View } from 'react-native';
import { dateFormat, useLang, useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { Logo } from '@/components/ui/text';

export function TodayHeader() {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  useLang();
  const date = dateFormat({ weekday: 'long', day: 'numeric', month: 'long' }).format(new Date());
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
      <View style={{ gap: 4, flexShrink: 1 }}>
        <Logo />
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
          {date.charAt(0).toUpperCase() + date.slice(1)}
        </Text>
      </View>
      <Pressable accessibilityRole="button" accessibilityLabel={tr('vote.header.profile')} onPress={() => router.push('/profile')}>
        <Avatar member={me ?? undefined} size={36} />
      </Pressable>
    </View>
  );
}
