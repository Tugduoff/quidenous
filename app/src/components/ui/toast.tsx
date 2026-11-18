import { CircleAlert } from 'lucide-react-native';
import { useEffect } from 'react';
import { Pressable, Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { motion } from '@/lib/motion';
import { dismissToast, useToast } from '@/lib/toast';
import { useTheme } from '@/lib/theme';

const SHOWN_MS = 3500;

export function ToastHost() {
  const t = useTheme();
  const insets = useSafeAreaInsets();
  const toast = useToast();
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => dismissToast(toast.id), SHOWN_MS);
    return () => clearTimeout(timer);
  }, [toast]);
  if (!toast) return null;
  return (
    <Animated.View
      key={toast.id}
      entering={motion.rise}
      exiting={motion.exit}
      pointerEvents="box-none"
      style={{ position: 'absolute', top: insets.top + 8, left: 16, right: 16, alignItems: 'center' }}>
      <Pressable
        accessibilityRole="alert"
        accessibilityLiveRegion="polite"
        onPress={() => dismissToast()}
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 10,
          maxWidth: 480,
          paddingHorizontal: 16,
          paddingVertical: 12,
          borderRadius: t.radius.pill,
          backgroundColor: t.card,
          borderWidth: 1,
          borderColor: t.line,
          ...t.shadow,
        }}>
        <CircleAlert size={18} color={t.tagInk} />
        <Text style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{toast.text}</Text>
      </Pressable>
    </Animated.View>
  );
}
