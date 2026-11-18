import { ArrowRight, Coins } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';
import { type TodayRoom } from '@/lib/api/today';
import { TAG_BG } from '@/components/ui/tag';

export function StatePill({ room: r, line }: { room: TodayRoom; line: string }) {
  const t = useTheme();
  if (r.state === 'vote' || r.state === 'result') {
    const vote = r.state === 'vote';
    const ink = vote ? t.accTxt : '#fff';
    return (
      <View
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 6,
          height: 32,
          paddingHorizontal: 14,
          borderRadius: t.radius.pill,
          backgroundColor: vote ? t.acc : TAG_BG,
        }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: ink }}>{line}</Text>
        <ArrowRight size={16} color={ink} strokeWidth={2.5} />
      </View>
    );
  }
  const tripot = r.state === 'tripot';
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, flexShrink: 1 }}>
      {tripot && <Coins size={15} color={t.accInk} />}
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: tripot ? t.accInk : t.mut }}>
        {line}
      </Text>
    </View>
  );
}
