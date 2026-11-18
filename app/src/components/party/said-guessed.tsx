import { Check } from 'lucide-react-native';
import { type ReactNode, useMemo } from 'react';
import { Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { LateCard } from '@/components/party/late-card';
import { Countdown, VotedLine } from '@/components/party/party';
import { GuessRecap } from '@/components/said/guess-recap';
import { MyAnswer } from '@/components/said/my-answer';
import { Card } from '@/components/ui/card';
import { Screen } from '@/components/ui/screen';
import type { Party, PartyRound } from '@/lib/api/party';
import type { SaidView } from '@/lib/api/rounds';
import { usePartyPlayers } from '@/lib/hooks/use-party-players';
import { usePhaseClock } from '@/lib/hooks/use-phase-clock';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { phaseSeconds } from '@/lib/utils/party';
import { toGuess } from '@/lib/utils/said';

/** Guesses sent (or arrived too late to play this one): the clock, who has guessed, and what I said. */
export function PartySaidGuessed({ party, round, said, top, late }: { party: Party; round: PartyRound; said: SaidView; top: ReactNode; late: boolean }) {
  const t = useTheme();
  const tr = useT();
  const ms = usePhaseClock(party, round);
  const byId = usePartyPlayers(party);
  const answers = useMemo(() => toGuess(said), [said]);
  return (
    <Screen tabs={false}>
      {top}
      <Countdown ms={ms} total={phaseSeconds(round, party.timer)} />
      <Animated.View entering={motion.enter} style={{ gap: 16 }}>
        {late ? (
          <LateCard />
        ) : (
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <Check size={20} color={t.accInk} strokeWidth={3} />
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
              {tr('said.party.sent')}
            </Text>
          </Card>
        )}
        <VotedLine round={round} byId={byId} label="said.party.guessed" />
        {!late && <GuessRecap answers={answers} guesses={said.my_guesses ?? []} byId={byId} />}
        {!late && said.my_answer && <MyAnswer text={said.my_answer.text} />}
      </Animated.View>
    </Screen>
  );
}
