import { type SharedValue, useSharedValue } from 'react-native-reanimated';

export function useSideScrollTouch(): SharedValue<boolean> {
  return useSharedValue(false);
}
