import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useState } from 'react';
import { View } from 'react-native';

import { Celebration } from '@/components/celebration';
import { isMilestone } from '@/lib/milestones';
import { useMembers, useRound } from '@/lib/queries';

import { Result } from '../(tabs)/index';

/** A finished question, opened from the history. A milestone question replays its party. */
export default function RoundScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const { byId } = useMembers();
  const { data: round } = useRound(Number(id));
  const [party, setParty] = useState(false);
  // Opened from a link or a reload there is nothing to go back to: land on the history.
  const back = () => (router.canGoBack() ? router.back() : router.replace('/history'));

  useEffect(() => {
    if ((round?.result || round?.choose) && isMilestone(round.number)) setParty(true);
  }, [round?.result, round?.choose, round?.number]);

  return (
    <View style={{ flex: 1 }}>
      <Result roundId={Number(id)} byId={byId} onBack={back} />
      {party && round && <Celebration count={round.number} onDone={() => setParty(false)} />}
    </View>
  );
}
