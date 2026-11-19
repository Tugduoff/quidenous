import { Text } from 'react-native';
import { useTheme } from '@/lib/theme';

export function Label({ children, k = 1 }: { children: string; k?: number }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.bold, fontSize: Math.round(11 * k), letterSpacing: 0.6 * k, color: t.mut }}>{children}</Text>;
}
