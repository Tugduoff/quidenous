import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Bell, BellOff } from 'lucide-react-native';
import Animated from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { type RoomSummary, roomKeys, setRoomMuted } from '@/lib/api/rooms';

export function MuteButton({ id, muted }: { id: number; muted: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const pop = usePop(muted);
  const toggle = useMutation({
    mutationFn: (mute: boolean) => setRoomMuted(id, mute),
    onMutate: async (mute) => {
      await qc.cancelQueries({ queryKey: roomKeys.all });
      const before = qc.getQueryData<RoomSummary[]>(roomKeys.all);
      qc.setQueryData<RoomSummary[]>(roomKeys.all, (rooms) => rooms?.map((r) => (r.id === id ? { ...r, muted: mute } : r)));
      return { before };
    },
    onError: (_e, _mute, ctx) => {
      if (ctx?.before) qc.setQueryData(roomKeys.all, ctx.before);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: roomKeys.all }),
  });
  const Icon = muted ? BellOff : Bell;
  return (
    <PressableScale
      accessibilityRole="switch"
      accessibilityLabel={tr('room.group.notifications')}
      accessibilityState={{ checked: !muted }}
      onPress={() => toggle.mutate(!muted)}
      hitSlop={6}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: muted ? t.card2 : t.card }}>
      <Animated.View style={pop}>
        <Icon size={20} color={muted ? t.mut : t.txt} />
      </Animated.View>
    </PressableScale>
  );
}
