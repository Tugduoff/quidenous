import { useLocalSearchParams } from 'expo-router';

import { useMembers } from '@/lib/queries';

import { Result } from '../(tabs)/index';

export default function RoundScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const { byId } = useMembers();
  return <Result roundId={Number(id)} byId={byId} />;
}
