import { photoUrl } from '@/lib/api/client';
import { useTheme } from '@/lib/theme';
import { parseAvatar } from '@/lib/utils/avatar';
import { View, Text } from 'react-native';
import { Image } from 'expo-image';
import { AvatarArt } from '@/components/ui/avatar-art';
import type { ReactNode } from 'react';
import Animated, { LayoutAnimationConfig } from 'react-native-reanimated';
import { motion } from '@/lib/motion';
import type { Member } from '@/lib/api/group';

export function Avatar({ member, size = 40 }: { member?: Member; size?: number }) {
  const t = useTheme();
  const uri = member && photoUrl(member);
  const avatar = !uri && parseAvatar(member?.avatar) ? member?.avatar : null;
  return (
    <View
      style={{
        width: size,
        height: size,
        borderRadius: size / 2,
        backgroundColor: member?.color ?? t.card2,
        alignItems: 'center',
        justifyContent: 'center',
        overflow: 'hidden',
      }}>
      {uri ? (
        <Image source={{ uri }} style={{ width: size, height: size }} />
      ) : avatar ? (
        <AvatarArt avatar={avatar} size={size} />
      ) : (
        <Text style={{ fontFamily: t.font.bold, fontSize: size * 0.42, color: '#121216' }}>{member?.name[0]?.toUpperCase() ?? '?'}</Text>
      )}
    </View>
  );
}

const stackOverlap = (size: number) => Math.round(size * 0.3);

export function avatarStackWidth(count: number, size: number, max?: number) {
  const n = max ? Math.min(count, max) : count;
  return n * (size + 4) - (n - 1) * stackOverlap(size);
}

export function AvatarStack({ members, size = 28, ring, max }: { members: (Member | undefined)[]; size?: number; ring?: string; max?: number }) {
  const t = useTheme();
  const shown = max && members.length > max ? members.slice(0, max - 1) : members;
  const extra = members.length - shown.length;
  const overlap = stackOverlap(size);
  const cell = (key: string | number, i: number, child: ReactNode) => (
    <Animated.View
      key={key}
      entering={motion.pop}
      style={{ marginLeft: i ? -overlap : 0, borderRadius: size, borderWidth: 2, borderColor: ring ?? t.bg }}>
      {child}
    </Animated.View>
  );
  return (
    <LayoutAnimationConfig skipEntering>
      <View style={{ flexDirection: 'row' }}>
        {shown.map((m, i) => cell(m?.id ?? i, i, <Avatar member={m} size={size} />))}
        {extra > 0 &&
          cell(
            'extra',
            shown.length,
            <View
              style={{
                width: size,
                height: size,
                borderRadius: size / 2,
                backgroundColor: t.mut,
                alignItems: 'center',
                justifyContent: 'center',
              }}>
              <Text style={{ fontFamily: t.font.bold, fontSize: size * 0.36, color: t.card }}>+{extra}</Text>
            </View>,
          )}
      </View>
    </LayoutAnimationConfig>
  );
}
