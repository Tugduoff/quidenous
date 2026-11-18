import { Globe } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** Under the threshold: how many rooms answered so far, out of how many it takes. */
export function NotEnoughRooms({ rooms, min }: { rooms: number; min: number }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Globe size={18} color={t.mut} />
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('globalQuestion.notEnough')}</Text>
      </View>
      <View style={{ height: 4, borderRadius: 2, backgroundColor: t.card2, overflow: 'hidden' }}>
        <View style={{ width: `${Math.min(100, (rooms / Math.max(min, 1)) * 100)}%`, height: 4, borderRadius: 2, backgroundColor: t.acc }} />
      </View>
      <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('globalQuestion.roomsSoFar', { count: rooms, min })}</Text>
    </View>
  );
}
