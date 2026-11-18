import { Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

/** The question closed with fewer than two answers: nothing to guess. */
export function SaidTooFew({ size, big }: { size: number; big?: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.enter} style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
      <Mascot who="roupi" mood="sleepy" size={size} />
      <Text
        style={{
          flex: 1,
          fontFamily: big ? t.font.display : t.font.semibold,
          fontSize: big ? Math.round(size * 0.3) : 16,
          lineHeight: big ? Math.round(size * 0.36) : 22,
          color: big ? t.txt : t.mut,
        }}>
        {tr('said.result.tooFew')}
      </Text>
    </Animated.View>
  );
}
