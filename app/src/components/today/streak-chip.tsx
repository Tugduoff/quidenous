import { Flame } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { HOT_FROM } from '@/lib/utils/streaks';

export function StreakChip({ days, risk }: { days: number; risk: boolean }) {
  const t = useTheme();
  const tr = useT();
  const color = risk ? t.tagInk : days >= HOT_FROM ? t.accInk : t.mut;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 3 }}>
      <Flame size={14} color={color} strokeWidth={2.5} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color }}>{risk ? `${days} · ${tr('today.atRisk')}` : days}</Text>
    </View>
  );
}
