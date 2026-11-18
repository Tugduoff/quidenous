import { router } from 'expo-router';
import { CalendarHeart, ChevronRight } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';

export function WrappedPreviewLink() {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale
      scale={0.98}
      accessibilityRole="button"
      accessibilityLabel={`${tr('wrapped.adminPreview')}. ${tr('wrapped.adminPreviewLine')}`}
      onPress={() => router.push('/wrapped?preview=1')}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
        <CalendarHeart size={20} color={t.txt} />
        <View style={{ flex: 1, gap: 2 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('wrapped.adminPreview')}</Text>
          <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('wrapped.adminPreviewLine')}</Text>
        </View>
        <ChevronRight size={20} color={t.mut} />
      </Card>
    </PressableScale>
  );
}
