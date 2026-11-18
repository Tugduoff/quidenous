import { useMutation, useQueryClient } from '@tanstack/react-query';
import { RotateCcw } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { BIG_GROUP, gridColumns, MemberGrid, MemberTile } from '@/components/vote/member-grid';
import { Screen } from '@/components/ui/screen';
import { BetRow } from '@/components/tripot/tripot';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { ordinal } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar, AvatarStack } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { type Member, groupKeys } from '@/lib/api/group';
import { type Round, roundKeys, voteCurrent } from '@/lib/api/rounds';
import { Question } from '@/components/vote/question';
import { Header } from '@/components/vote/header';
import { Pop } from '@/components/vote/pop';
import { Springy } from '@/components/vote/springy';
import { VoteTip } from '@/components/vote/vote-tip';

/** Ranking question: tap everyone in order, first to last. Tap a ranked person again to take them out. */
export function RankBallot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [picked, setOrder] = useState<number[]>([]);
  // Only one person left to place: there is only one place left for them, no need to tap.
  const last = members.length > 1 && picked.length === members.length - 1 ? members.find((m) => !picked.includes(m.id)) : undefined;
  const order = last ? [...picked, last.id] : picked;
  const vote = useMutation({
    mutationFn: (ranking: number[]) => voteCurrent({ ranking }),
    onSuccess: onVoted,
    // Someone joined or left meanwhile: the room refreshed shows who to vote on now (the error is a toast).
    onError: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
    onSettled: () => qc.invalidateQueries({ queryKey: roundKeys.current }),
  });
  const toggle = (id: number) => setOrder(order.includes(id) ? order.filter((x) => x !== id) : [...order, id]);
  const done = order.length === members.length;
  const next = order.length + 1;
  const sorted = [...order.map((id) => members.find((m) => m.id === id)), ...members.filter((m) => !order.includes(m.id))].filter(
    (m): m is Member => !!m,
  );
  return (
    <Screen
      footer={
        <Springy trigger={done} when={done}>
          <Button disabled={!done} loading={vote.isPending} onPress={() => done && vote.mutate(order)}>
            {done ? tr('vote.rank.submit') : tr('vote.rank.whoIs', { place: ordinal(next) })}
          </Button>
        </Springy>
      }>
      <Header />
      <Question round={round} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} max={6} />
        <Text style={{ flex: 1, fontFamily: t.font.regular, color: t.mut }}>
          {round.voter_ids.length}/{round.member_count}
        </Text>
        {order.length > 0 && (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('vote.rank.restart')}
            onPress={() => setOrder([])}
            hitSlop={8}
            style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
            <RotateCcw size={18} color={t.txt} />
          </PressableScale>
        )}
      </View>
      <BetRow round={round} members={members} byId={byId} />
      <VoteTip />
      {/* Ranked people move up to their place, the rest slide down. A big group gets a grid of faces (no endless list). */}
      {members.length > BIG_GROUP + 1 ? (
        <MemberGrid>
          {sorted.map((m) => {
            const place = order.indexOf(m.id) + 1;
            return (
              <MemberTile
                key={m.id}
                member={m}
                cols={gridColumns(members.length)}
                selected={place > 0}
                badge={place || undefined}
                label={place ? tr('vote.rank.ranked', { name: m.name, place }) : tr('vote.rank.add', { name: m.name })}
                onPress={() => toggle(m.id)}
              />
            );
          })}
        </MemberGrid>
      ) : (
        <View style={{ gap: 10 }}>
          {sorted.map((m) => {
            const place = order.indexOf(m.id) + 1;
            return (
              <Animated.View key={m.id} layout={motion.layout}>
                <PressableScale
                  scale={0.98}
                  accessibilityRole="button"
                  accessibilityLabel={place ? tr('vote.rank.ranked', { name: m.name, place }) : tr('vote.rank.add', { name: m.name })}
                  onPress={() => toggle(m.id)}
                  style={[
                    {
                      flexDirection: 'row',
                      alignItems: 'center',
                      gap: 12,
                      padding: 12,
                      borderRadius: 18,
                      borderWidth: 2,
                      borderColor: place ? t.accInk : 'transparent',
                      backgroundColor: place ? t.accSoft : t.card,
                    },
                    t.shadow,
                  ]}>
                  <Pop on={place > 0} to={1.2}>
                    <View
                      style={{
                        width: 32,
                        height: 32,
                        borderRadius: 16,
                        alignItems: 'center',
                        justifyContent: 'center',
                        backgroundColor: place ? t.acc : t.card2,
                      }}>
                      <Text style={{ fontFamily: t.font.display, fontSize: 16, color: place ? t.accTxt : t.mut }}>{place || '·'}</Text>
                    </View>
                  </Pop>
                  <Avatar member={m} size={36} />
                  <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }} numberOfLines={1}>
                    {m.name}
                  </Text>
                </PressableScale>
              </Animated.View>
            );
          })}
        </View>
      )}
    </Screen>
  );
}
