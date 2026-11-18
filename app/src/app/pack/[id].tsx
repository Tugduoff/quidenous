import { useLocalSearchParams } from 'expo-router';
import { PackView } from '@/components/packs/pack-view';

/** One pack: its questions, like, install (into a room I pick when I have several), share, report; edit and delete (author); feature (admin). */
export default function PackScreen() {
  const id = Number(useLocalSearchParams<{ id: string }>().id);
  return <PackView id={id} />;
}
