import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { BIG_GROUP, gridColumns, MemberGrid, MemberTile } from '@/components/vote/member-grid';
import { Screen } from '@/components/ui/screen';
import { BetRow } from '@/components/tripot/tripot';
import { useT } from '@/lib/i18n';
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

export function PickBallot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [pick, setPick] = useState<number | null>(null);
  const vote = useMutation({
    mutationFn: (target_id: number) => voteCurrent({ target_id }),
    onSuccess: onVoted,
    onError: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
    onSettled: () => qc.invalidateQueries({ queryKey: roundKeys.current }),
  });
  const picked = pick != null ? byId.get(pick) : undefined;
  return (
    <Screen
      footer={
        <Springy trigger={pick} when={pick != null}>
          <Button disabled={!picked} loading={vote.isPending} onPress={() => pick != null && vote.mutate(pick)}>
            {picked ? (
              <Text numberOfLines={1} style={{ flexShrink: 1, paddingHorizontal: 20, fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>
                {tr('vote.voteFor', { name: picked.name })}
              </Text>
            ) : (
              tr('party.vote.pick')
            )}
          </Button>
        </Springy>
      }>
      <Header />
      <Question round={round} />
      {round.voter_ids.length > 0 && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} max={6} />
          <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
            {round.voter_ids.length}/{round.member_count}
          </Text>
        </View>
      )}
      <BetRow round={round} members={members} byId={byId} />
      <VoteTip />
      {members.length > BIG_GROUP ? (
        <MemberGrid>
          {members.map((m) => (
            <MemberTile
              key={m.id}
              member={m}
              cols={gridColumns(members.length)}
              role="radio"
              selected={m.id === pick}
              onPress={() => setPick(m.id)}
            />
          ))}
        </MemberGrid>
      ) : (
        <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
          {members.map((m) => {
            const on = m.id === pick;
            return (
              <PressableScale
                key={m.id}
                scale={0.97}
                accessibilityRole="radio"
                accessibilityState={{ selected: on }}
                onPress={() => setPick(m.id)}
                style={[
                  {
                    width: '48%',
                    flexDirection: 'row',
                    alignItems: 'center',
                    gap: 10,
                    padding: 14,
                    borderRadius: 20,
                    borderWidth: 2,
                    borderColor: on ? t.accInk : 'transparent',
                    backgroundColor: on ? t.accSoft : t.card,
                  },
                  t.shadow,
                ]}>
                <Pop on={on}>
                  <Avatar member={m} />
                </Pop>
                <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt, flex: 1 }} numberOfLines={1}>
                  {m.name}
                </Text>
              </PressableScale>
            );
          })}
        </View>
      )}
    </Screen>
  );
}
