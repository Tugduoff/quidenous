import { Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';

export function PointsPill({ points, k = 1 }: { points: number; k?: number }) {
  const t = useTheme();
  const on = points > 0;
  return (
    <View
      style={{
        minWidth: 52 * k,
        height: 40 * k,
        paddingHorizontal: 12 * k,
        borderRadius: 20 * k,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: on ? t.acc : t.card2,
      }}>
      <Text style={{ fontFamily: t.font.display, fontSize: Math.round(20 * k), color: on ? t.accTxt : t.mut }}>+{points}</Text>
    </View>
  );
}
