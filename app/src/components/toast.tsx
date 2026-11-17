import { CircleAlert } from 'lucide-react-native';
import { useEffect, useSyncExternalStore } from 'react';
import { Pressable, Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

const SHOWN_MS = 3500;

type Toast = { id: number; text: string };
let current: Toast | null = null;
let seq = 0;
const listeners = new Set<() => void>();
const set = (next: Toast | null) => {
  current = next;
  listeners.forEach((l) => l());
};

/** A short message floating at the top for a few seconds (an action that failed). The same text again just restarts it. */
export function showToast(text: string) {
  set({ id: ++seq, text });
}

export function ToastHost() {
  const t = useTheme();
  const insets = useSafeAreaInsets();
  const toast = useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => current,
  );
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => current?.id === toast.id && set(null), SHOWN_MS);
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
        onPress={() => set(null)}
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
