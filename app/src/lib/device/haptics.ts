import * as Haptics from 'expo-haptics';
import { Platform } from 'react-native';

export type ImpactStrength = 'light' | 'medium' | 'heavy';

const STYLES: Record<ImpactStrength, Haptics.ImpactFeedbackStyle> = {
  light: Haptics.ImpactFeedbackStyle.Light,
  medium: Haptics.ImpactFeedbackStyle.Medium,
  heavy: Haptics.ImpactFeedbackStyle.Heavy,
};

const HAS_HAPTICS = Platform.OS !== 'web';

export function impactHaptic(strength: ImpactStrength = 'light') {
  if (HAS_HAPTICS) Haptics.impactAsync(STYLES[strength]).catch(() => {});
}

export function selectionHaptic() {
  if (HAS_HAPTICS) Haptics.selectionAsync().catch(() => {});
}
