import type { ReactNode } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

export function GlobalSection({ title, right, children }: { title: string; right?: string; children: ReactNode }) {
  const t = useTheme();
  return (
    <Animated.View entering={motion.rise} style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'baseline', justifyContent: 'space-between', marginHorizontal: 4 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{title}</Text>
        {!!right && <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{right}</Text>}
      </View>
      {children}
    </Animated.View>
  );
}
