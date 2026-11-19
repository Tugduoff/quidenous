import type { ReactNode } from 'react';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

export function RoundButton({
  label,
  size,
  onPress,
  accent,
  soft,
  children,
}: {
  label: string;
  size: number;
  onPress: () => void;
  accent?: boolean;
  soft?: boolean;
  children: ReactNode;
}) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      onPress={onPress}
      hitSlop={6}
      style={[
        {
          width: size,
          height: size,
          borderRadius: size / 2,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: accent ? t.acc : soft ? t.card2 : t.card,
        },
        !soft && t.shadow,
      ]}>
      {children}
    </PressableScale>
  );
}
