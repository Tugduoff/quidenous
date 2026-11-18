import { Linking, Pressable, Text, View } from 'react-native';
import { useDonateUrl } from '@/lib/api/donate';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** "Buy us a coffee": a quiet link to the donation page in the browser, when there is one (lib/donate.ts). */
export function DonateLink() {
  const t = useTheme();
  const tr = useT();
  const url = useDonateUrl();
  if (!url) return null;
  return (
    <View style={{ alignItems: 'center', gap: 2, paddingHorizontal: 24 }}>
      <Pressable accessibilityRole="link" onPress={() => Linking.openURL(url).catch(() => {})} hitSlop={8}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.txt }}>{tr('profile.donate.title')}</Text>
      </Pressable>
      <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut, textAlign: 'center' }}>{tr('profile.donate.line')}</Text>
    </View>
  );
}
