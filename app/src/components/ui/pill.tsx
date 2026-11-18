import { useTheme } from '@/lib/theme';
import { Pressable, ActivityIndicator, Text } from 'react-native';

export function Pill({
  children,
  onPress,
  loading,
  disabled,
  tone = 'soft',
}: {
  children: string;
  onPress: () => void;
  loading?: boolean;
  disabled?: boolean;
  tone?: 'soft' | 'acc' | 'danger';
}) {
  const t = useTheme();
  const acc = tone === 'acc' && !disabled;
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={children}
      onPress={onPress}
      disabled={disabled || loading}
      hitSlop={4}
      style={({ pressed }) => ({
        height: 36,
        paddingHorizontal: 14,
        borderRadius: 999,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: acc ? t.acc : t.card2,
        opacity: pressed ? 0.8 : 1,
      })}>
      {loading ? (
        <ActivityIndicator size="small" color={acc ? t.accTxt : t.mut} />
      ) : (
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 14, color: acc ? t.accTxt : tone === 'danger' ? '#FF3B5C' : disabled ? t.mut : t.txt }}>
          {children}
        </Text>
      )}
    </Pressable>
  );
}
