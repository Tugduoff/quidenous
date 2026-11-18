import { useLocalSearchParams } from 'expo-router';
import { PackView } from '@/components/packs/pack-view';

export default function PackScreen() {
  const id = Number(useLocalSearchParams<{ id: string }>().id);
  return <PackView id={id} />;
}
