import { Flame, Users } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { HOT_FROM, SHOWN_FROM, useMemberStreak, useStreaks } from '@/lib/api/streaks';
import { useTheme } from '@/lib/theme';
import { Caps, int, Section } from '@/components/stats/common';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar, AvatarStack } from '@/components/ui/avatar';
import type { Member } from '@/lib/api/group';
import type { Stats } from '@/lib/api/stats';

/** A flame and a number: muted, in the accent colour from a week. */
function FlameCount({ n, size = 13, label }: { n: number; size?: number; label: string }) {
  const t = useTheme();
  const color = n >= HOT_FROM ? t.accInk : t.mut;
  return (
    <View accessible accessibilityLabel={label} style={{ flexDirection: 'row', alignItems: 'center', gap: 2 }}>
      <Flame size={size} color={color} strokeWidth={2.5} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: size - 1, color }}>{n}</Text>
    </View>
  );
}

/** Next to a name: someone's streak in this room, from 2 days (nothing on a server without streaks). */
export function StreakBadge({ id }: { id: number }) {
  const tr = useT();
  const n = useMemberStreak(id);
  if (n < SHOWN_FROM) return null;
  return <FlameCount n={n} label={tr('streaks.a11y', { count: n })} />;
}

/** Next to the room's name on the Vote tab: the room's streak (everyone voted), from 2 days. */
export function RoomStreak() {
  const tr = useT();
  const n = useStreaks()?.room?.current ?? 0;
  if (n < SHOWN_FROM) return null;
  return (
    <Animated.View entering={motion.enter}>
      <FlameCount n={n} label={tr('streaks.roomA11y', { count: n })} />
    </Animated.View>
  );
}

/** Profile, under my name: my streak in this room and my record. */
export function MyStreak() {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const s = useStreaks()?.members[String(me?.id ?? 0)];
  if (!s || (s.current < 1 && s.best < SHOWN_FROM)) return null;
  const color = s.current >= HOT_FROM ? t.accInk : t.mut;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
      <Flame size={15} color={color} strokeWidth={2.5} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color }}>{tr('streaks.mine', { count: s.current })}</Text>
      {s.best > s.current && (
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>· {tr('streaks.best', { count: s.best })}</Text>
      )}
    </View>
  );
}

/** One record card, in the style of the awards. */
function Record({
  title,
  who,
  quip,
  n,
  people,
  onPress,
  index,
}: {
  title: string;
  who: string;
  quip: string;
  n: number;
  people: (Member | undefined)[];
  onPress?: () => void;
  index: number;
}) {
  const t = useTheme();
  const tr = useT();
  const one = people.length === 1;
  return (
    <Animated.View entering={motion.stagger(index)}>
      <PressableScale
        scale={0.98}
        disabled={!onPress}
        accessibilityRole={onPress ? 'button' : undefined}
        accessibilityLabel={`${title}: ${who}, ${tr('streaks.a11y', { count: n })}`}
        onPress={onPress}
        style={[
          { flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14, borderRadius: t.radius.card, backgroundColor: t.card },
          t.shadow,
        ]}>
        {one ? (
          <Avatar member={people[0]} size={44} />
        ) : people.length ? (
          <AvatarStack members={people} size={34} ring={t.card} max={3} />
        ) : (
          <View style={{ width: 44, height: 44, borderRadius: 22, backgroundColor: t.card2, alignItems: 'center', justifyContent: 'center' }}>
            <Users size={20} color={t.mut} />
          </View>
        )}
        <View style={{ flex: 1, gap: 3 }}>
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
            <Flame size={13} color={n >= HOT_FROM ? t.accInk : t.mut} strokeWidth={2.5} />
            <Caps size={11} lines={2}>
              {title}
            </Caps>
          </View>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
            {who}
          </Text>
          <Text numberOfLines={2} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 17, color: t.mut }}>
            {quip}
          </Text>
        </View>
        <View style={{ alignItems: 'flex-end', maxWidth: 96 }}>
          <Text numberOfLines={1} style={{ fontFamily: t.font.display, fontSize: 26, letterSpacing: -0.5, color: t.txt }}>
            {int(n)}
          </Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 11, color: t.mut }}>
            {tr('streaks.stats.days', { count: n })}
          </Text>
        </View>
      </PressableScale>
    </Animated.View>
  );
}

/** Stats: the longest streak among the members and the room's best (all time), when there is one of 2 days or more. */
export function StreakRecords({
  s,
  byId,
  members,
  onOpen,
}: {
  s: Stats;
  byId: Map<number, Member>;
  members: Member[];
  onOpen: (id: number) => void;
}) {
  const tr = useT();
  const d = s.day_streaks;
  const member = d?.member;
  const room = d && d.room.best >= SHOWN_FROM ? d.room : null;
  if (!member && !room) return null;
  return (
    <Section title={tr('streaks.stats.section')}>
      <View style={{ gap: 10 }}>
        {member && (
          <Record
            index={0}
            title={tr('streaks.stats.member')}
            who={byId.get(member.member_id)?.name ?? '?'}
            quip={tr('streaks.stats.memberQuip')}
            n={member.value}
            people={[byId.get(member.member_id)]}
            onPress={() => onOpen(member.member_id)}
          />
        )}
        {room && (
          <Record
            index={1}
            title={tr('streaks.stats.room')}
            who={tr('streaks.stats.roomName')}
            quip={room.current >= SHOWN_FROM ? tr('streaks.stats.now', { count: room.current }) : tr('streaks.stats.roomQuip')}
            n={room.best}
            people={members}
          />
        )}
      </View>
    </Section>
  );
}
