import { Check } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import type { RoomSummary } from '@/lib/api/rooms';

export function PackInstallDialog({
  visible,
  onClose,
  rooms,
  isIn,
  busy,
  onPick,
}: {
  visible: boolean;
  onClose: () => void;
  rooms: RoomSummary[] | undefined;
  isIn: (roomId: number) => boolean;
  busy: boolean;
  onPick: (roomId: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  return (
  <Dialog visible={visible} onClose={onClose}>
    <DialogTitle title={tr('packs.installIn')} />
    <View style={{ gap: 8 }}>
      {rooms?.map((r) => {
        const done = isIn(r.id);
        return (
          <PressableScale
            key={r.id}
            accessibilityRole="button"
            disabled={busy}
            onPress={() => {
              onClose();
              onPick(r.id);
            }}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 10,
              height: 48,
              paddingHorizontal: 14,
              borderRadius: 14,
              backgroundColor: t.card2,
            }}>
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
              {r.name}
            </Text>
            {done && <Check size={18} color={t.accInk} accessibilityLabel={tr('packs.installedHere')} />}
          </PressableScale>
        );
      })}
    </View>
  </Dialog>
  );
}
