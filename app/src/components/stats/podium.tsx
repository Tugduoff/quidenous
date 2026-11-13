import { Crown } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Avatar, PressableScale } from '@/components/ui';
import type { Member, MemberStats } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

const STEP = [104, 78, 60];
const FACE = [60, 48, 44];

/** Top three by votes received (then wins), on steps: second, first, third. Needs two people with votes. */
export function podiumOf(profiles: MemberStats[]) {
  const ranked = profiles.filter((p) => p.received > 0).sort((a, b) => b.received - a.received || b.wins - a.wins || a.member_id - b.member_id);
  return ranked.length >= 2 ? ranked.slice(0, 3) : [];
}

export function Podium({ top, byId, onOpen }: { top: MemberStats[]; byId: Map<number, Member>; onOpen: (id: number) => void }) {
  const t = useTheme();
  const tr = useT();
  // Places on screen: 2nd on the left, 1st in the middle, 3rd on the right.
  const order = top.length === 3 ? [1, 0, 2] : [1, 0];
  return (
    <View style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 10, paddingTop: 8 }}>
      {order.map((place) => {
        const p = top[place];
        const m = byId.get(p.member_id);
        const color = m?.color ?? t.mut;
        return (
          <Animated.View key={p.member_id} entering={motion.stagger(place)} style={{ flex: 1 }}>
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel={`${place + 1}, ${m?.name ?? '?'}, ${tr('stats.podiumVotes', { count: p.received })}`}
              onPress={() => onOpen(p.member_id)}
              style={{ alignItems: 'center', gap: 8 }}>
              {place === 0 && <Crown size={20} color={color} />}
              <Avatar member={m} size={FACE[place]} />
              <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 14, color: t.txt, maxWidth: '100%' }}>
                {m?.name ?? '?'}
              </Text>
              <View
                style={{
                  alignSelf: 'stretch',
                  height: STEP[place],
                  borderTopLeftRadius: 16,
                  borderTopRightRadius: 16,
                  backgroundColor: color,
                  alignItems: 'center',
                  justifyContent: 'center',
                }}>
                <Text style={{ fontFamily: t.font.display, fontSize: place === 0 ? 30 : 24, color: '#121216' }}>{p.received}</Text>
                <Text style={{ fontFamily: t.font.semibold, fontSize: 11, color: '#121216B3' }}>{tr('stats.unit.votes', { count: p.received })}</Text>
              </View>
            </PressableScale>
          </Animated.View>
        );
      })}
    </View>
  );
}
