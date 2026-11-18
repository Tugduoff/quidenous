import type { ReactNode } from 'react';
import { Text } from 'react-native';
import { FactRows } from '@/components/global/global';
import { GlobalHeading } from '@/components/global/global-heading';
import { GlobalSection } from '@/components/global/global-section';
import { NotEnoughRooms } from '@/components/global/not-enough-rooms';
import { YesterdayWinner } from '@/components/global/yesterday-winner';
import { ResultBars } from '@/components/result/result-bars';
import { Screen } from '@/components/ui/screen';
import { type GlobalCount, type GlobalOpen } from '@/lib/api/global';
import { useT } from '@/lib/i18n';
import { tidy } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { useMembers } from '@/lib/api/group';

const bars = (tally: GlobalCount[]) => tally.map((c) => ({ ...c, voter_ids: [] }));

export function GlobalResults({ view, top }: { view: GlobalOpen; top: ReactNode }) {
  const t = useTheme();
  const tr = useT();
  const { byId } = useMembers();
  const { global, room, yesterday } = view;
  return (
    <Screen tabs={false}>
      {top}
      <GlobalHeading question={view.question} />
      <GlobalSection title={tr('globalQuestion.myRoom')} right={tr('globalQuestion.roomVoters', { voters: room.voters, members: room.members })}>
        <ResultBars result={bars(room.tally ?? [])} byId={byId} />
      </GlobalSection>
      <GlobalSection title={tr('globalQuestion.everywhere')} right={global.facts ? tr('globalQuestion.rooms', { count: global.rooms }) : undefined}>
        <Card style={{ padding: 16 }}>
          {global.facts ? <FactRows facts={global.facts.slice(0, 3)} /> : <NotEnoughRooms rooms={global.rooms} min={global.min_rooms} />}
        </Card>
      </GlobalSection>
      {yesterday && (
        <GlobalSection title={tr('globalQuestion.yesterday')}>
          <Card style={{ padding: 16, gap: 14 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 21, color: t.txt }}>{tidy(yesterday.question)}</Text>
            <YesterdayWinner tally={yesterday.tally} byId={byId} />
            {yesterday.facts && <FactRows facts={yesterday.facts.slice(0, 2)} />}
          </Card>
        </GlobalSection>
      )}
    </Screen>
  );
}
