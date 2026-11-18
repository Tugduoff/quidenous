import { Check, X } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

/** Reject (or remove) and approve, side by side. */
export function ReviewActions({ busy, onReject, rejectLabel, onApprove }: { busy: boolean; onReject: () => void; rejectLabel: string; onApprove: () => void }) {
  const t = useTheme();
  const tr = useT();
  const pill = {
    flex: 1,
    height: 42,
    borderRadius: t.radius.pill,
    flexDirection: 'row' as const,
    gap: 6,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
  };
  return (
    <View style={{ flexDirection: 'row', gap: 10, marginTop: 4 }}>
      <PressableScale accessibilityRole="button" disabled={busy} onPress={onReject} style={[pill, { backgroundColor: t.card2 }]}>
        <X size={16} color={t.tagInk} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{rejectLabel}</Text>
      </PressableScale>
      <PressableScale accessibilityRole="button" disabled={busy} onPress={onApprove} style={[pill, { backgroundColor: t.acc }]}>
        <Check size={16} color={t.accTxt} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: t.accTxt }}>{tr('admin.approve')}</Text>
      </PressableScale>
    </View>
  );
}
