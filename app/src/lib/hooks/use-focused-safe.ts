import { useIsFocused } from 'expo-router';

export function useFocusedSafe() {
  try {
    return useIsFocused();
  } catch {
    return true;
  }
}
