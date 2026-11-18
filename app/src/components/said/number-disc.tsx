import { Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';

export function NumberDisc({ n, on, size = 26 }: { n: number; on?: boolean; size?: number }) {
  const t = useTheme();
  return (
    <View
      style={{
        width: size,
        height: size,
        borderRadius: size / 2,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: on ? t.acc : t.card2,
      }}>
      <Text style={{ fontFamily: t.font.display, fontSize: Math.round(size / 2), color: on ? t.accTxt : t.mut }}>{n}</Text>
    </View>
  );
}
