import { RotateCcw } from 'lucide-react-native';
import { Pressable, Text } from 'react-native';
import { useTheme } from '@/lib/theme';

export function AgainButton({ label, onPress }: { label: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      hitSlop={8}
      style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 6, minHeight: 44, paddingHorizontal: 12 }}>
      <RotateCcw size={14} color={t.mut} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{label}</Text>
    </Pressable>
  );
}
