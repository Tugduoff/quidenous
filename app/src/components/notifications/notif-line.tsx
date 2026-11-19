import type { LucideIcon } from 'lucide-react-native';
import type { ReactNode } from 'react';
import { Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';

export function NotifLine({
  Icon,
  on,
  title,
  sub,
  warn,
  right,
}: {
  Icon: LucideIcon;
  on: boolean;
  title: string;
  sub?: string;
  warn?: boolean;
  right: ReactNode;
}) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12, minHeight: 36 }}>
      <Icon size={20} color={on ? t.accInk : t.mut} />
      <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {title}
        </Text>
        {!!sub && <Text numberOfLines={1} style={{ fontFamily: warn ? t.font.semibold : t.font.regular, fontSize: 13, color: warn ? t.tagInk : t.mut }}>{sub}</Text>}
      </View>
      {right}
    </View>
  );
}

export function Dim({ off, children }: { off: boolean; children: ReactNode }) {
  return (
    <View aria-disabled={off} style={{ opacity: off ? 0.4 : 1, pointerEvents: off ? 'none' : 'auto' }}>
      {children}
    </View>
  );
}
