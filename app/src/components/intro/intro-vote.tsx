import { useState } from 'react';
import { Text, View } from 'react-native';
import { useReducedMotion } from 'react-native-reanimated';
import { IntroCandidate } from '@/components/intro/intro-candidate';
import { IntroProgress } from '@/components/intro/intro-progress';
import { useTimeout } from '@/lib/hooks/use-timeout';
import { useT } from '@/lib/i18n';
import { DEMO_ORDER } from '@/lib/utils/intro';
import { CAST, type Who } from '@/lib/utils/mascot';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { QuestionText } from '@/components/ui/text';

const VOTE_MS = 380;

export function IntroVote({ pick, onPick, onCounted }: { pick: Who | null; onPick: (w: Who) => void; onCounted: () => void }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const total = 1 + DEMO_ORDER.length;
  const [count, setCount] = useState(pick ? total : 0);
  const choose = (who: Who) => {
    onPick(who);
    setCount(1);
  };
  const all = count >= total;
  useTimeout(
    () => (all ? onCounted() : setCount(count + 1)),
    !pick || count === 0 ? null : all ? (reduced ? 250 : 450) : reduced ? 0 : VOTE_MS,
    count,
  );

  return (
    <>
      <Card style={{ padding: 18, gap: 10 }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.accInk, textTransform: 'uppercase' }}>
          {tr('intro.vote.eyebrow')}
        </Text>
        <QuestionText text={tr('intro.vote.question')} rank={false} style={{ fontFamily: t.font.display, fontSize: 26, lineHeight: 29, letterSpacing: -0.6, color: t.txt }} />
      </Card>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10, justifyContent: 'center' }}>
        {CAST.map((who) => (
          <IntroCandidate key={who} who={who} picked={pick === who} dim={!!pick && pick !== who} onPress={pick ? undefined : () => choose(who)} />
        ))}
      </View>
      <View style={{ alignItems: 'center', gap: 10, minHeight: 40 }} accessibilityLiveRegion="polite">
        {pick ? (
          <>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('intro.vote.counting', { n: Math.max(1, count) })}</Text>
            <IntroProgress value={count / (1 + DEMO_ORDER.length)} />
          </>
        ) : (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('intro.vote.hint')}</Text>
        )}
      </View>
    </>
  );
}
