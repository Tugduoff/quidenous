import { View } from 'react-native';
import Animated, { type SharedValue, useAnimatedStyle } from 'react-native-reanimated';
import { INK } from '@/components/wrapped/constants';

/** One thin bar per card: the past ones full, the current one filling (on the UI thread), the next ones empty. */
export function StorySegment({ i, index, progress }: { i: number; index: number; progress: SharedValue<number> }) {
  const fill = useAnimatedStyle(() => ({ width: `${(i < index ? 1 : i > index ? 0 : progress.value) * 100}%` }));
  return (
    <View style={{ flex: 1, height: 3, borderRadius: 2, backgroundColor: '#1212162E', overflow: 'hidden' }}>
      <Animated.View style={[{ height: 3, borderRadius: 2, backgroundColor: INK }, fill]} />
    </View>
  );
}
