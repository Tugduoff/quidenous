import { router, useLocalSearchParams } from 'expo-router';

import { useMembers } from '@/lib/queries';

import { Result } from '../(tabs)/index';

/** A finished question, opened from the history. */
export default function RoundScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const { byId } = useMembers();
  // Opened from a link or a reload there is nothing to go back to: land on the history.
  const back = () => (router.canGoBack() ? router.back() : router.replace('/history'));
  return <Result roundId={Number(id)} byId={byId} onBack={back} />;
}
