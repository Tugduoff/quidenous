import { Check, Download } from 'lucide-react-native';
import { Text } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

export function PackInstallButton({ disabled, several, installedHere, onPress }: { disabled: boolean; several: boolean; installedHere: boolean; onPress: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale
      accessibilityRole="button"
      disabled={disabled}
      onPress={onPress}
      style={{
        height: 52,
        paddingHorizontal: 20,
        borderRadius: t.radius.pill,
        flexDirection: 'row',
        gap: 8,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: installedHere && !several ? t.card2 : t.acc,
      }}>
      {installedHere && !several ? <Check size={18} color={t.accInk} /> : <Download size={18} color={t.accTxt} />}
      <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 16, color: installedHere && !several ? t.accInk : t.accTxt }}>
        {several ? tr('packs.installIn') : installedHere ? tr('packs.installedHere') : tr('packs.install')}
      </Text>
    </PressableScale>
  );
}
