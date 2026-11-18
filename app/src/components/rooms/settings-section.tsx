import { ChevronRight, type LucideIcon } from 'lucide-react-native';
import type { ReactNode } from 'react';
import { Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';

/** A small titled block: grey header with its icon (red for the danger zone), the rows in a card. */
export function SettingsSection({ title, Icon, danger, children }: { title: string; Icon: LucideIcon; danger?: boolean; children: ReactNode }) {
  const t = useTheme();
  const color = danger ? t.tagInk : t.mut;
  return (
    <View style={{ gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, marginLeft: 4 }}>
        <Icon size={14} color={color} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color }}>{title}</Text>
      </View>
      <Card style={{ padding: 16, gap: 14 }}>{children}</Card>
    </View>
  );
}

/** One setting on one line: its label, then a control (or, with `onPress`, a value and a chevron). */
export function SettingsRow({
  label,
  value,
  right,
  onPress,
  a11y,
  danger,
}: {
  label: string;
  value?: string;
  right?: ReactNode;
  onPress?: () => void;
  a11y?: string;
  danger?: boolean;
}) {
  const t = useTheme();
  const body = (
    <>
      <Text numberOfLines={1} style={{ flexShrink: 0, maxWidth: '70%', fontFamily: t.font.semibold, fontSize: 15, color: danger ? t.tagInk : t.txt }}>
        {label}
      </Text>
      <View style={{ flex: 1, minWidth: 0, flexDirection: 'row', alignItems: 'center', justifyContent: 'flex-end', gap: 8 }}>
        {value != null && (
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
            {value}
          </Text>
        )}
        {right ?? (onPress && <ChevronRight size={18} color={danger ? t.tagInk : t.mut} />)}
      </View>
    </>
  );
  const style = { flexDirection: 'row', alignItems: 'center', gap: 12, minHeight: 36 } as const;
  if (!onPress) return <View style={style}>{body}</View>;
  return (
    <PressableScale scale={0.98} accessibilityRole="button" accessibilityLabel={a11y ?? label} onPress={onPress} style={style}>
      {body}
    </PressableScale>
  );
}
