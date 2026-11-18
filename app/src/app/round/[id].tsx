import { router, useLocalSearchParams } from 'expo-router';
import { useState } from 'react';
import { View } from 'react-native';
import { Celebration } from '@/components/celebration';
import { isMilestone } from '@/lib/utils/milestones';
import { Result } from '@/components/result/result';
import { useMembers } from '@/lib/api/group';
import { useRound } from '@/lib/api/rounds';

/** A finished question, opened from the history. A milestone question replays its party. */
export default function RoundScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const { byId } = useMembers();
  const { data: round } = useRound(Number(id));
  const [partyDone, setPartyDone] = useState<number | null>(null);
  // Opened from a link or a reload there is nothing to go back to: land on the history.
  const back = () => (router.canGoBack() ? router.back() : router.replace('/history'));

  const party = !!(round?.result || round?.choose || round?.said?.result) && !!round && isMilestone(round.number) && partyDone !== round.number;

  return (
    <View style={{ flex: 1 }}>
      <Result roundId={Number(id)} byId={byId} onBack={back} />
      {party && round && <Celebration count={round.number} onDone={() => setPartyDone(round.number)} />}
    </View>
  );
}
