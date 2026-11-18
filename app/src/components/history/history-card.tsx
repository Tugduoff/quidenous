import { router } from 'expo-router';
import { Cable, Crown, MessageSquareQuote, PartyPopper, Pin, Target } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AnonymousMark } from '@/components/vote/anonymous';
import { useT } from '@/lib/i18n';
import { isMilestone } from '@/lib/utils/milestones';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar, AvatarStack, avatarStackWidth } from '@/components/ui/avatar';
import { QuestionText } from '@/components/ui/text';
import { type Member } from '@/lib/api/group';
import { type HistoryItem } from '@/lib/api/history';

const SOLO = 44;
const TIED = 40;
const MAX = 2;

export function HistoryCard({ item, byId, order }: { item: HistoryItem; byId: Map<number, Member>; order?: number }) {
  const t = useTheme();
  const tr = useT();
  const winners = item.winner_ids.map((id) => byId.get(id));
  const winner = winners[0];
  const choose = item.kind === 'choose';
  const said = item.kind === 'said';
  const Mark = choose ? Target : Crown;
  const Empty = said ? MessageSquareQuote : Cable;
  const lead = winners.length > 1 ? avatarStackWidth(winners.length, TIED, MAX) : SOLO;
  return (
    <Animated.View entering={order != null ? motion.stagger(order) : undefined}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        onPress={() => router.push(`/round/${item.id}`)}
        style={[{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14, borderRadius: 20, backgroundColor: t.card }, t.shadow]}>
        <View style={{ width: lead, alignItems: 'center' }}>
          {winners.length > 1 ? (
            <AvatarStack members={winners} size={TIED} ring={t.card} max={MAX} />
          ) : winner || !(choose || said) ? (
            <Avatar member={winner} size={SOLO} />
          ) : (
            <View
              style={{ width: SOLO, height: SOLO, borderRadius: SOLO / 2, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
              <Empty size={20} color={t.mut} />
            </View>
          )}
        </View>
        <View style={{ flex: 1, gap: 4 }}>
          <QuestionText
            text={item.question}
            rank={item.kind === 'rank'}
            choose={choose}
            said={said}
            style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 19, color: t.txt }}
          />
          <View style={{ flexDirection: 'row', alignItems: 'flex-start', gap: 4 }}>
            <Mark size={14} color={winners.length === 1 ? (winner?.color ?? t.mut) : t.mut} style={{ flexShrink: 0, marginTop: 1 }} />
            <Text
              numberOfLines={1}
              style={{ flex: item.anonymous ? undefined : 1, flexShrink: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>
              {winners.length ? winners.map((m) => m?.name).join(', ') : choose ? tr('history.nobodyGuessed') : said ? tr('said.result.nobody') : ''}
            </Text>
            {item.anonymous && (
              <View style={{ marginTop: 1.5, marginLeft: 2 }}>
                <AnonymousMark size={13} />
              </View>
            )}
          </View>
        </View>
        {(item.pinned || isMilestone(item.number)) && (
          <View style={{ alignSelf: 'flex-start', alignItems: 'flex-end', gap: 6, flexShrink: 0 }}>
            {item.pinned && <Pin size={16} color={t.accInk} />}
            {isMilestone(item.number) && (
              <View
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 4,
                  paddingHorizontal: 8,
                  height: 22,
                  borderRadius: t.radius.pill,
                  backgroundColor: t.accSoft,
                }}>
                <PartyPopper size={12} color={t.accInk} />
                <Text style={{ fontFamily: t.font.bold, fontSize: 11, color: t.accInk }}>#{item.number}</Text>
              </View>
            )}
          </View>
        )}
      </PressableScale>
    </Animated.View>
  );
}
