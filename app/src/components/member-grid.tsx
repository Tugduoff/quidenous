import type { ReactNode } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import type { Member } from '@/lib/api';
import { motion, usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

import { Avatar, PressableScale } from './ui';

/** Above this many people, ballots switch from wide cards to a grid of faces. */
export const BIG_GROUP = 6;

/** Columns of the grid of faces: three for a mid-size group, four from 13 people. */
export const gridColumns = (count: number) => (count <= 12 ? 3 : 4);

/** A grid of faces (wraps; children are MemberTiles with the same `cols`). Cells keep 8 px between them. */
export function MemberGrid({ children }: { children: ReactNode }) {
  return <View style={{ flexDirection: 'row', flexWrap: 'wrap', margin: -4 }}>{children}</View>;
}

/** One person in a MemberGrid: the face, the name under it on one line, an optional badge on the corner. */
export function MemberTile({
  member,
  cols,
  selected,
  badge,
  plain,
  dim,
  label,
  role = 'button',
  onPress,
}: {
  member: Member;
  cols: number;
  selected?: boolean;
  /** A number or an icon in a small disc on the top right corner. */
  badge?: ReactNode;
  /** Inside a card: just the face and the name, smaller, no background. */
  plain?: boolean;
  dim?: boolean;
  label?: string;
  role?: 'button' | 'radio';
  onPress: () => void;
}) {
  const t = useTheme();
  const pop = usePop(selected, 1.15, !!selected);
  const size = plain ? 40 : cols >= 4 ? 40 : 46;
  return (
    <Animated.View layout={motion.layout} style={{ width: `${100 / cols}%`, padding: 4 }}>
      <PressableScale
        scale={0.96}
        accessibilityRole={role}
        accessibilityLabel={label ?? member.name}
        accessibilityState={role === 'radio' ? { selected: !!selected } : undefined}
        onPress={onPress}
        style={[
          {
            alignItems: 'center',
            gap: plain ? 4 : 6,
            paddingVertical: plain ? 4 : 10,
            paddingHorizontal: plain ? 0 : 4,
            borderRadius: 18,
            borderWidth: 2,
            borderColor: selected ? t.accInk : 'transparent',
            backgroundColor: selected ? t.accSoft : plain ? 'transparent' : t.card,
            opacity: dim ? 0.45 : 1,
          },
          !plain && t.shadow,
        ]}>
        <Animated.View style={pop}>
          <Avatar member={member} size={size} />
        </Animated.View>
        <Text
          numberOfLines={1}
          style={{ alignSelf: 'stretch', textAlign: 'center', fontFamily: t.font.semibold, fontSize: plain ? 12 : 13, color: t.txt }}>
          {member.name}
        </Text>
        {badge != null && (
          <View
            style={{
              position: 'absolute',
              top: plain ? 0 : 4,
              right: plain ? 2 : 4,
              minWidth: 22,
              height: 22,
              paddingHorizontal: 4,
              borderRadius: 11,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: selected ? t.acc : t.card2,
            }}>
            {typeof badge === 'number' || typeof badge === 'string' ? (
              <Text style={{ fontFamily: t.font.display, fontSize: 12, color: selected ? t.accTxt : t.mut }}>{badge}</Text>
            ) : (
              badge
            )}
          </View>
        )}
      </PressableScale>
    </Animated.View>
  );
}
