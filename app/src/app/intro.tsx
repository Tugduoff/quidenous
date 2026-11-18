// The first-run intro: la bande shows a new account how a round goes, then on to a first room (lib/intro.ts for the rules,
// docs/TODAY.md "Intro"). Four short steps: hello, a practice vote on the Petons, its verdict, your turn. Skippable at any time.
import { type ReactNode, useCallback, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { IntroDots } from '@/components/intro/intro-dots';
import { IntroEnd } from '@/components/intro/intro-end';
import { IntroHello } from '@/components/intro/intro-hello';
import { IntroVerdict } from '@/components/intro/intro-verdict';
import { IntroVote } from '@/components/intro/intro-vote';
import { Screen } from '@/components/ui/screen';
import { Button } from '@/components/ui/button';
import { useHardwareBack } from '@/lib/hooks/use-hardware-back';
import { useT } from '@/lib/i18n';
import { type Who } from '@/lib/utils/mascot';
import { motion } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

type Step = 'hello' | 'vote' | 'verdict' | 'end';
const STEPS: Step[] = ['hello', 'vote', 'verdict', 'end'];


export default function Intro() {
  const tr = useT();
  const { endIntro, roomless } = useSession();
  const [step, setStep] = useState<Step>('hello');
  const [pick, setPick] = useState<Who | null>(null);
  const i = STEPS.indexOf(step);
  const counted = useCallback(() => setStep('verdict'), []);

  // Back a step (Android's button; the first step leaves the app, as any first screen). Back to the practice vote
  // (from its verdict or the end): a fresh ballot.
  useHardwareBack(() => {
    const prev = STEPS[i - 1] === 'verdict' ? 'vote' : STEPS[i - 1];
    if (prev === 'vote') setPick(null);
    setStep(prev);
  }, i > 0);

  const footer =
    step === 'hello' ? (
      <Button testID="intro-start" onPress={() => setStep('vote')}>
        {tr('intro.hello.cta')}
      </Button>
    ) : step === 'verdict' ? (
      <Button testID="intro-next" onPress={() => setStep('end')}>
        {tr('intro.next')}
      </Button>
    ) : step === 'end' ? (
      <Button testID="intro-done" onPress={() => endIntro(false)}>
        {tr(roomless ? 'intro.end.cta' : 'intro.end.inCta')}
      </Button>
    ) : undefined;

  return (
    <Screen tabs={false} footer={footer}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', minHeight: 44 }}>
        <IntroDots index={i} count={STEPS.length} />
        {step !== 'end' && (
          <Pressable testID="intro-skip" accessibilityRole="button" onPress={() => endIntro(true)} hitSlop={10} style={{ padding: 8 }}>
            <SkipText>{tr('intro.skip')}</SkipText>
          </Pressable>
        )}
      </View>
      <Animated.View key={step} entering={motion.question} style={{ flex: 1, gap: 20 }}>
        {step === 'hello' && <IntroHello />}
        {step === 'vote' && <IntroVote pick={pick} onPick={setPick} onCounted={counted} />}
        {step === 'verdict' && pick && <IntroVerdict pick={pick} />}
        {step === 'end' && <IntroEnd roomless={roomless} />}
      </Animated.View>
    </Screen>
  );
}

function SkipText({ children }: { children: ReactNode }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{children}</Text>;
}
