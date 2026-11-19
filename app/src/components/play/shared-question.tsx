import { Swords } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Tag } from '@/components/ui/tag';
import { Display } from '@/components/ui/text';
import type { PlayInfo } from '@/lib/api/community';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { tidy } from '@/lib/utils/text';

export function SharedQuestion({ q }: { q: PlayInfo }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.question} style={{ gap: 12 }}>
      <Tag label={tr(q.kind === 'rank' ? 'vote.kind.rank' : q.kind === 'choose' ? 'vote.kind.choose' : 'vote.kind.pick')} />
      <Display size={30}>{tidy(q.question)}</Display>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
        {q.closed && <Swords size={14} color={t.mut} />}
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {q.closed ? tr('community.play.challenge') : tr('community.play.from', { room: q.room })}
        </Text>
      </View>
    </Animated.View>
  );
}
