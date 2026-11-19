import { Text } from 'react-native';
import { useTheme } from '@/lib/theme';

export function SectionTitle({ children }: { children: string }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>{children}</Text>;
}
