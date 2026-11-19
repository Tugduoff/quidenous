import type { ReactNode } from 'react';
import { type ViewStyle, View } from 'react-native';
import { useTheme } from '@/lib/theme';

export function Card({ children, style }: { children: ReactNode; style?: ViewStyle }) {
  const t = useTheme();
  return <View style={[{ backgroundColor: t.card, borderRadius: t.radius.card }, t.shadow, style]}>{children}</View>;
}
