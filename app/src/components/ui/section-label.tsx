import { Text } from 'react-native';
import { useTheme } from '@/lib/theme';

export function SectionLabel({ children }: { children: string }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{children}</Text>;
}
