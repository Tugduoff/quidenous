import type { ReactNode } from 'react';
import { useTheme } from '@/lib/theme';
import { View, Text } from 'react-native';
import { Card } from '@/components/ui/card';

export function Section({ title, children }: { title: string; children: ReactNode }) {
  const t = useTheme();
  return (
    <View style={{ gap: 8 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{title}</Text>
      <Card style={{ padding: 16, gap: 14 }}>{children}</Card>
    </View>
  );
}

export function Row({ icon, title, sub, right }: { icon?: ReactNode; title: string; sub?: string | null; right?: ReactNode }) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12, minHeight: 36 }}>
      {icon}
      <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {title}
        </Text>
        {!!sub && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
            {sub}
          </Text>
        )}
      </View>
      {right}
    </View>
  );
}
