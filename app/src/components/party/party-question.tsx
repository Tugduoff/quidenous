import Animated from 'react-native-reanimated';
import { Tag } from '@/components/ui/tag';
import { Display } from '@/components/ui/text';
import type { PartyRound } from '@/lib/api/party';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { tidy } from '@/lib/utils/text';

export function PartyQuestion({ round, size }: { round: PartyRound; size: number }) {
  const tr = useT();
  return (
    <Animated.View entering={motion.question} style={{ gap: 12 }}>
      <Tag label={round.kind === 'said' ? `#${round.number} · ${tr('said.reveal.title')}` : `#${round.number}`} />
      <Display size={size}>{tidy(round.question)}</Display>
    </Animated.View>
  );
}
