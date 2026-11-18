import { View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { pickLine } from '@/lib/utils/said';

/** Zeste, the instigator, eggs you on from the side: a line for the start, along the way, and once all is placed. */
export function ZesteSays({ seed, done, total }: { seed: number; done: number; total: number }) {
  const t = useTheme();
  const tr = useT();
  const stage = done === 0 ? 's' : done >= total ? 'd' : 'm';
  const lines =
    stage === 's' ? (['s1', 's2', 's3', 's4'] as const) : stage === 'd' ? (['d1', 'd2', 'd3'] as const) : (['m1', 'm2', 'm3', 'm4'] as const);
  const line = pickLine(lines, seed * 31 + done);
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
      <Mascot
        who="zeste"
        mood={stage === 's' ? 'think' : stage === 'd' ? 'happy' : 'accuse'}
        size={64}
        style={{ marginVertical: -6, marginLeft: -6 }}
      />
      {/* The bubble's tail, then the bubble. */}
      <View
        style={{
          width: 0,
          height: 0,
          borderTopWidth: 7,
          borderBottomWidth: 7,
          borderRightWidth: 9,
          borderTopColor: 'transparent',
          borderBottomColor: 'transparent',
          borderRightColor: t.card,
          marginRight: -4,
        }}
      />
      <View style={[{ flex: 1, paddingVertical: 10, paddingHorizontal: 14, borderRadius: 16, backgroundColor: t.card }, t.shadow]}>
        <Animated.Text key={line} entering={motion.enter} style={{ fontFamily: t.font.semibold, fontSize: 14, lineHeight: 19, color: t.txt }}>
          {tr(`said.zeste.${line}`)}
        </Animated.Text>
      </View>
    </View>
  );
}
