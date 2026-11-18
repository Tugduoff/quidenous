import { ChevronRight } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Dialog, DialogActions, DialogTitle } from '@/components/ui/dialog';
import type { Member } from '@/lib/api/group';

export function HandOverDialog({
  visible,
  members,
  busy,
  onClose,
  onConfirm,
}: {
  visible: boolean;
  members: Member[];
  busy: boolean;
  onClose: () => void;
  onConfirm: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const [heir, setHeir] = useState<Member | null>(null);
  const [wasVisible, setWasVisible] = useState(visible);
  if (visible !== wasVisible) {
    setWasVisible(visible);
    if (visible) setHeir(null);
  }
  return (
    <Dialog visible={visible} onClose={onClose}>
      {heir ? (
        <>
          <DialogTitle title={tr('room.settings.handOverAsk')} message={tr('room.settings.handOverMessage', { name: heir.name })} />
          <DialogActions confirmLabel={tr('common.confirm')} busy={busy} onConfirm={() => onConfirm(heir.id)} onCancel={() => setHeir(null)} />
        </>
      ) : (
        <>
          <DialogTitle title={tr('room.settings.handOver')} />
          <View style={{ gap: 6 }}>
            {members.map((m) => (
              <PressableScale
                key={m.id}
                scale={0.98}
                accessibilityRole="button"
                accessibilityLabel={tr('room.settings.makeOwner', { name: m.name })}
                onPress={() => setHeir(m)}
                style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 10, borderRadius: 16, backgroundColor: t.card2 }}>
                <Avatar member={m} size={32} />
                <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {m.name}
                </Text>
                <ChevronRight size={18} color={t.mut} />
              </PressableScale>
            ))}
          </View>
        </>
      )}
    </Dialog>
  );
}
