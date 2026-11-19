import { Download } from 'lucide-react-native';
import { View } from 'react-native';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Display } from '@/components/ui/text';
import { downloadHistory } from '@/lib/device/share';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function HistoryHeader({ canExport }: { canExport: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
      <Display size={30}>{tr('history.title')}</Display>
      {canExport && (
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('history.export')}
          onPress={downloadHistory}
          hitSlop={8}
          style={[{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
          <Download size={18} color={t.txt} />
        </PressableScale>
      )}
    </View>
  );
}
