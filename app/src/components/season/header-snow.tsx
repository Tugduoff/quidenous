import { View } from 'react-native';
import { useSeason } from '@/components/season/season-scope';
import { Snow } from '@/components/season/snow';

const ABOVE_THE_QUESTION_PX = 150;
const PAGE_PADDING = 20;

export function HeaderSnow() {
  if (useSeason() !== 'winter') return null;
  return (
    <View
      pointerEvents="none"
      style={{ position: 'absolute', top: -PAGE_PADDING, left: -PAGE_PADDING, right: -PAGE_PADDING, height: ABOVE_THE_QUESTION_PX, overflow: 'hidden' }}>
      <Snow height={ABOVE_THE_QUESTION_PX} />
    </View>
  );
}
