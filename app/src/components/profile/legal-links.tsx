import { Linking, Pressable, Text, View } from 'react-native';
import { API_URL } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function LegalLinks({ lang }: { lang: 'fr' | 'en' }) {
  const t = useTheme();
  const tr = useT();
  const open = (page: 'privacy' | 'terms') => Linking.openURL(`${API_URL}${lang === 'en' ? '/en' : ''}/${page}`).catch(() => {});
  const small = { fontFamily: t.font.semibold, fontSize: 12, color: t.mut };
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'center', alignItems: 'center', gap: 6 }}>
      <Pressable accessibilityRole="link" onPress={() => open('privacy')} hitSlop={8}>
        <Text style={small}>{tr('legal.privacy')}</Text>
      </Pressable>
      <Text style={small}>·</Text>
      <Pressable accessibilityRole="link" onPress={() => open('terms')} hitSlop={8}>
        <Text style={small}>{tr('legal.terms')}</Text>
      </Pressable>
    </View>
  );
}
