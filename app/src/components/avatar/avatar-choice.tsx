import type { ReactNode } from 'react';
import { View } from 'react-native';
import { AvatarArt } from '@/components/ui/avatar-art';
import type { Avatar } from '@/lib/utils/avatar';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

/** Close-up on the face, for the small features. */
const ZOOM = '20 24 60 60';

export function AvatarCircle({ size, children }: { size: number; children: ReactNode }) {
  return <View style={{ width: size, height: size, borderRadius: size / 2, overflow: 'hidden' }}>{children}</View>;
}

/** One choice: the avatar as it would be with it. */
export function AvatarChoice({
  avatar,
  on,
  onPress,
  width,
  zoom,
  label,
}: {
  avatar: Avatar;
  on: boolean;
  onPress: () => void;
  width: number;
  zoom?: boolean;
  label: string;
}) {
  const t = useTheme();
  const inner = Math.round(width - 16);
  return (
    <PressableScale
      accessibilityRole="radio"
      accessibilityLabel={label}
      accessibilityState={{ selected: on }}
      onPress={onPress}
      style={[
        { width, height: width, borderRadius: 22, alignItems: 'center', justifyContent: 'center', borderWidth: 2 },
        { backgroundColor: on ? t.accSoft : t.card, borderColor: on ? t.accInk : 'transparent' },
        t.shadow,
      ]}>
      <AvatarCircle size={inner}>
        <AvatarArt avatar={avatar} size={inner} viewBox={zoom ? ZOOM : undefined} />
      </AvatarCircle>
    </PressableScale>
  );
}
