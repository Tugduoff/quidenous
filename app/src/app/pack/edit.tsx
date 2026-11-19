import { useLocalSearchParams } from 'expo-router';
import { PackEditor } from '@/components/packs/pack-editor';

export default function PackEditScreen() {
  const param = useLocalSearchParams<{ id?: string }>().id;
  const id = param ? Number(param) : null;
  return <PackEditor id={id} />;
}
