import { useLocalSearchParams } from 'expo-router';
import { PackEditor } from '@/components/packs/pack-editor';

/** Publish a pack (`/pack/edit`, prefilled with this room's own questions) or edit one of mine (`/pack/edit?id=`). */
export default function PackEditScreen() {
  const param = useLocalSearchParams<{ id?: string }>().id;
  const id = param ? Number(param) : null;
  return <PackEditor id={id} />;
}
