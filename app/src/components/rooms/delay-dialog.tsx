import { Pressable, Text, View } from 'react-native';
import { t as tr, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Dialog, DialogTitle } from '@/components/ui/dialog';

const DELAYS = [0, 900, 3600, 10800, 43200, 86400];
export const delayLabel = (s: number) =>
  !s
    ? tr('room.settings.delayNone')
    : s < 3600
      ? tr('common.time.minutes', { count: Math.round(s / 60) })
      : tr('room.settings.hours', { count: Math.round(s / 3600) });

export function DelayDialog({ visible, value, onClose, onPick }: { visible: boolean; value: number; onClose: () => void; onPick: (s: number) => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={tr('room.settings.delay')} />
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
        {DELAYS.map((s) => {
          const on = s === value;
          return (
            <Pressable
              key={s}
              accessibilityRole="radio"
              accessibilityState={{ selected: on }}
              onPress={() => onPick(s)}
              style={{
                width: '30%',
                flexGrow: 1,
                height: 44,
                borderRadius: 12,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: on ? t.acc : t.card2,
              }}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: on ? t.accTxt : t.txt }}>{delayLabel(s)}</Text>
            </Pressable>
          );
        })}
      </View>
    </Dialog>
  );
}
