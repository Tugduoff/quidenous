import { Lightbulb, X } from 'lucide-react-native';
import { Pressable, Text } from 'react-native';
import Animated from 'react-native-reanimated';

import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { dismissTip, useTipVisible } from '@/lib/prefs';
import { useTheme } from '@/lib/theme';

/** A one-line first-run hint with an X; once closed it never comes back (kept per device). */
export function Tip({ id, text }: { id: string; text: string }) {
  const t = useTheme();
  const tr = useT();
  if (!useTipVisible(id)) return null;
  return (
    <Animated.View
      entering={motion.enter}
      exiting={motion.exit}
      style={{
        flexDirection: 'row',
        alignItems: 'center',
        gap: 10,
        paddingVertical: 10,
        paddingLeft: 14,
        paddingRight: 8,
        borderRadius: 16,
        backgroundColor: t.accSoft,
      }}>
      <Lightbulb size={16} color={t.accInk} />
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 13, lineHeight: 18, color: t.txt }}>{text}</Text>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('vote.tipClose')}
        onPress={() => dismissTip(id)}
        hitSlop={8}
        style={{ width: 28, height: 28, borderRadius: 14, alignItems: 'center', justifyContent: 'center' }}>
        <X size={16} color={t.mut} />
      </Pressable>
    </Animated.View>
  );
}
