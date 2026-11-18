import type { ReactNode } from 'react';
import { type ViewStyle, Pressable, ActivityIndicator, Text } from 'react-native';
import { useTheme } from '@/lib/theme';

export function SoftButton({
  children,
  icon,
  onPress,
  loading,
  disabled,
  style,
}: {
  children: string;
  icon?: ReactNode;
  onPress: () => void;
  loading?: boolean;
  disabled?: boolean;
  style?: ViewStyle;
}) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={children}
      disabled={disabled || loading}
      onPress={onPress}
      style={({ pressed }) => [
        { height: 58, borderRadius: 999, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 10 },
        { backgroundColor: t.card, borderWidth: 1, borderColor: t.line, opacity: pressed || disabled ? 0.7 : 1 },
        style,
      ]}>
      {loading ? <ActivityIndicator color={t.mut} /> : icon}
      <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
        {children}
      </Text>
    </Pressable>
  );
}
