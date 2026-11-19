import { useMutation } from '@tanstack/react-query';
import { Check } from 'lucide-react-native';
import { useState } from 'react';
import { Text, TextInput, View } from 'react-native';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { REPORT_REASONS, type ReportReason, reportPack } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Dialog, DialogActions, DialogTitle } from '@/components/ui/dialog';

export function PackReportDialog({ visible, onClose, onSent, id }: { visible: boolean; onClose: () => void; onSent: () => void; id: number }) {
  const t = useTheme();
  const tr = useT();
  const [reason, setReason] = useState<ReportReason | null>(null);
  const [details, setDetails] = useState('');
  const send = useMutation({
    meta: { quiet: true },
    mutationFn: () => reportPack(id, reason, details.trim() || undefined),
    onSuccess: onSent,
    onError: (e) => (e instanceof ApiError && e.status === 409 ? onSent() : undefined),
  });
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={tr('packs.reportTitle')} />
      <View accessibilityRole="radiogroup" style={{ gap: 8 }}>
        {REPORT_REASONS.map((r) => {
          const on = r === reason;
          return (
            <PressableScale
              key={r}
              accessibilityRole="radio"
              accessibilityState={{ selected: on }}
              onPress={() => setReason(r)}
              style={{
                flexDirection: 'row',
                alignItems: 'center',
                height: 46,
                paddingHorizontal: 14,
                borderRadius: 14,
                borderWidth: 2,
                borderColor: on ? t.accInk : 'transparent',
                backgroundColor: on ? t.accSoft : t.card2,
              }}>
              <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr(`packs.reasons.${r}`)}</Text>
              {on && <Check size={18} color={t.accInk} />}
            </PressableScale>
          );
        })}
      </View>
      {reason === 'other' && (
        <TextInput
          accessibilityLabel={tr('packs.reportDetails')}
          placeholder={tr('packs.reportDetails')}
          placeholderTextColor={t.mut}
          value={details}
          onChangeText={setDetails}
          maxLength={300}
          style={{
            height: 44,
            paddingHorizontal: 14,
            borderRadius: 14,
            backgroundColor: t.card2,
            fontFamily: t.font.regular,
            fontSize: 16,
            color: t.txt,
          }}
        />
      )}
      {send.isError && !(send.error instanceof ApiError && send.error.status === 409) && (
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk }}>{tr('common.failed')}</Text>
      )}
      <DialogActions confirmLabel={tr('packs.report')} onConfirm={() => send.mutate()} onCancel={onClose} busy={send.isPending} disabled={!reason} />
    </Dialog>
  );
}
