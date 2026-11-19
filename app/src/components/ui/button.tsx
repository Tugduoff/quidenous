import type { ReactNode } from 'react';
import { type ViewStyle, ActivityIndicator, Text, StyleSheet } from 'react-native';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

export function Button({
  children,
  onPress,
  disabled,
  loading,
  style,
  testID,
}: {
  children: ReactNode;
  onPress?: () => void;
  disabled?: boolean;
  loading?: boolean;
  style?: ViewStyle;
  testID?: string;
}) {
  const t = useTheme();
  const off = disabled || loading;
  return (
    <PressableScale
      testID={testID}
      accessibilityRole="button"
      accessibilityState={{ disabled: !!off }}
      disabled={off}
      onPress={onPress}
      style={[styles.button, { backgroundColor: off ? t.card2 : t.acc }, style]}>
      {loading ? (
        <ActivityIndicator color={t.mut} />
      ) : typeof children === 'string' ? (
        <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: off ? t.mut : t.accTxt }}>{children}</Text>
      ) : (
        children
      )}
    </PressableScale>
  );
}

const styles = StyleSheet.create({
  button: { height: 58, borderRadius: 999, alignItems: 'center', justifyContent: 'center', flexDirection: 'row', gap: 8 },
});
