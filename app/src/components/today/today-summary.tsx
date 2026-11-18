import { memo } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { type TodayRoom } from '@/lib/api/today';
import { Display } from '@/components/ui/text';

/** The big line: how many questions wait for me, else "all caught up" with Roupi, who has nothing to do either. */
export const TodaySummary = memo(function TodaySummary({ rooms }: { rooms: TodayRoom[] }) {
  const t = useTheme();
  const tr = useT();
  const votes = rooms.filter((r) => r.state === 'vote').length;
  const results = rooms.filter((r) => r.state === 'result').length;
  const waiting = rooms.some((r) => r.state === 'wait' || r.state === 'tripot');
  const done = votes + results === 0;
  const title = votes ? tr('today.summary.vote', { count: votes }) : results ? tr('today.summary.results', { count: results }) : tr('today.summary.done');
  const sub = votes && results ? tr('today.summary.andResults', { count: results }) : done ? tr(waiting ? 'today.summary.doneWaiting' : 'today.summary.doneIdle') : null;
  return (
    <View accessibilityRole="header" style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 4 }}>
      <View style={{ flex: 1, gap: 6 }}>
        <Display size={30}>{title}</Display>
        {sub && <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{sub}</Text>}
      </View>
      {done && (
        <Animated.View entering={motion.enter}>
          <Mascot who="roupi" mood="sleepy" size={88} />
        </Animated.View>
      )}
    </View>
  );
});
