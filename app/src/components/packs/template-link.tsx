import { Sparkles } from 'lucide-react-native';
import { Text, type ViewStyle } from 'react-native';
import { useT } from '@/lib/i18n';
import { downloadPackTemplate } from '@/lib/device/share';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

export function TemplateLink({ style }: { style?: ViewStyle }) {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale
      accessibilityRole="link"
      onPress={() => downloadPackTemplate()}
      hitSlop={8}
      style={[{ flexDirection: 'row', alignItems: 'center', gap: 5 }, style]}>
      <Sparkles size={13} color={t.accInk} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.accInk }}>{tr('packs.template')}</Text>
    </PressableScale>
  );
}
