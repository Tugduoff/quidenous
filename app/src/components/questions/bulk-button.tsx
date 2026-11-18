import type { ReactNode } from 'react';
import { Text } from 'react-native';
import { PressableScale } from '@/components/ui/pressable-scale';
import { useTheme } from '@/lib/theme';

export function BulkButton({
  label,
  icon,
  onPress,
  disabled,
  accessibilityLabel,
}: {
  label: string;
  icon: ReactNode;
  onPress: () => void;
  disabled?: boolean;
  accessibilityLabel?: string;
}) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={accessibilityLabel ?? label}
      disabled={disabled}
      onPress={onPress}
      style={{
        flex: 1,
        height: 44,
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 8,
        borderRadius: t.radius.pill,
        backgroundColor: t.card,
        borderWidth: 1.5,
        borderColor: t.line,
        opacity: disabled ? 0.5 : 1,
      }}>
      {icon}
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>
        {label}
      </Text>
    </PressableScale>
  );
}
