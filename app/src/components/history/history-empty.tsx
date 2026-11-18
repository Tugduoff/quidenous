import { router } from 'expo-router';
import { Pressable, Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

/** No result yet: one line, and the way back to the vote. */
export function HistoryEmpty() {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.enter} style={{ flex: 1, alignItems: 'center', justifyContent: 'center', gap: 12, paddingBottom: 60 }}>
      {/* Roupi waits for the first results the only way he knows. */}
      <Mascot who="roupi" mood="sleepy" size={128} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut, textAlign: 'center' }}>{tr('history.empty')}</Text>
      <Pressable accessibilityRole="button" onPress={() => router.navigate('/')} hitSlop={8}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.accInk }}>{tr('history.goVote')}</Text>
      </Pressable>
    </Animated.View>
  );
}
