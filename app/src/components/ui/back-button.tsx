import { useTheme } from '@/lib/theme';
import { useT } from '@/lib/i18n';
import { Pressable } from 'react-native';
import { router } from 'expo-router';
import { ChevronLeft } from 'lucide-react-native';

export function BackButton({ onPress }: { onPress?: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={tr('common.back')}
      onPress={onPress ?? (() => (router.canGoBack() ? router.back() : router.replace('/')))}
      hitSlop={4}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
      <ChevronLeft size={22} color={t.txt} />
    </Pressable>
  );
}
