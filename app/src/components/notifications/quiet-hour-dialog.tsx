import { Pressable, Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { hhmm } from '@/lib/utils/time';

const HOURS = Array.from({ length: 24 }, (_, h) => h * 60);

export function QuietHourDialog({
  visible,
  which,
  pickedValue,
  otherValue,
  onPick,
  onClose,
}: {
  visible: boolean;
  which: 'start' | 'end';
  pickedValue: number | null;
  otherValue: number | null;
  onPick: (m: number) => void;
  onClose: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  return (
  <Dialog visible={visible} onClose={onClose}>
    <DialogTitle title={which === 'end' ? tr('notifications.end') : tr('notifications.start')} />
    <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
      {HOURS.map((m) => {
        const on = m === pickedValue;
        const taken = m === otherValue;
        return (
          <Pressable
            key={m}
            accessibilityRole="radio"
            accessibilityState={{ selected: on, disabled: taken }}
            disabled={taken}
            onPress={() => {
              if (!on) onPick(m);
              onClose();
            }}
            style={{
              width: '22%',
              flexGrow: 1,
              height: 40,
              borderRadius: 12,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: on ? t.acc : t.card2,
              opacity: taken ? 0.3 : 1,
            }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: on ? t.accTxt : t.txt }}>{hhmm(m)}</Text>
          </Pressable>
        );
      })}
    </View>
  </Dialog>
  );
}
