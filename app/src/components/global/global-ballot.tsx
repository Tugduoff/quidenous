import { type ReactNode, useState } from 'react';
import { Text } from 'react-native';
import { GlobalHeading } from '@/components/global/global-heading';
import { gridColumns, MemberGrid, MemberTile } from '@/components/vote/member-grid';
import { Screen } from '@/components/ui/screen';
import { type GlobalOpen, useGlobalVote } from '@/lib/api/global';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { useMembers } from '@/lib/api/group';

export function GlobalBallot({ view, top }: { view: GlobalOpen; top: ReactNode }) {
  const t = useTheme();
  const tr = useT();
  const { members } = useMembers();
  const [pick, setPick] = useState<number | null>(null);
  const vote = useGlobalVote();
  const picked = members.find((m) => m.id === pick);
  return (
    <Screen
      tabs={false}
      footer={
        <Button disabled={!picked} loading={vote.isPending} onPress={() => pick != null && vote.mutate(pick)}>
          {picked ? (
            <Text numberOfLines={1} style={{ flexShrink: 1, paddingHorizontal: 20, fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>
              {tr('vote.voteFor', { name: picked.name })}
            </Text>
          ) : (
            tr('party.vote.pick')
          )}
        </Button>
      }>
      {top}
      <GlobalHeading question={view.question} />
      <Text style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 19, color: t.mut }}>
        {tr('globalQuestion.forRoom')}
        {view.room.voters > 0 ? ` ${tr('globalQuestion.answered', { count: view.room.voters })}.` : ''}
      </Text>
      <MemberGrid>
        {members.map((m) => (
          <MemberTile key={m.id} member={m} cols={gridColumns(members.length)} role="radio" selected={m.id === pick} onPress={() => setPick(m.id)} />
        ))}
      </MemberGrid>
    </Screen>
  );
}
