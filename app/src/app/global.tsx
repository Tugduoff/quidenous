import { Globe } from 'lucide-react-native';
import { Redirect } from 'expo-router';
import { type ReactNode, useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { FactRows } from '@/components/global/global';
import { gridColumns, MemberGrid, MemberTile } from '@/components/vote/member-grid';
import { QueryState } from '@/components/ui/query-state';
import { ResultBars } from '@/components/result/result-bars';
import { Screen } from '@/components/ui/screen';
import { type GlobalCount, type GlobalOpen, useGlobal, useGlobalVote } from '@/lib/api/global';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { tidy } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';
import { type Member, useMembers } from '@/lib/api/group';

/** The global question of the day (docs/GLOBAL_QUESTION.md): vote for someone of my room, then my room's tally and the aggregates. */
export default function GlobalScreen() {
  const t = useTheme();
  const global = useGlobal();
  const view = global.data;
  const top = (
    <View style={{ flexDirection: 'row', alignItems: 'center' }}>
      <BackButton />
    </View>
  );
  if (!view) {
    return (
      <Screen tabs={false}>
        {top}
        {global.isError ? <QueryState query={global} /> : <ActivityIndicator color={t.mut} style={{ marginTop: 24 }} />}
      </Screen>
    );
  }
  // The room can't take it (any more): back to the home screen.
  if (!view.available) return <Redirect href="/" />;
  return view.my_vote == null ? <Ballot view={view} top={top} /> : <Results view={view} top={top} />;
}

function Heading({ question }: { question: string }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
        <Globe size={15} color={t.accInk} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.accInk }}>{tr('globalQuestion.label').toUpperCase()}</Text>
      </View>
      <Display size={30}>{tidy(question)}</Display>
    </View>
  );
}

function Ballot({ view, top }: { view: GlobalOpen; top: ReactNode }) {
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
      <Heading question={view.question} />
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

/** Counts per person (the bars of a result, without who voted). */
const bars = (tally: GlobalCount[]) => tally.map((c) => ({ ...c, voter_ids: [] }));

function Section({ title, right, children }: { title: string; right?: string; children: ReactNode }) {
  const t = useTheme();
  return (
    <Animated.View entering={motion.rise} style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'baseline', justifyContent: 'space-between', marginHorizontal: 4 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{title}</Text>
        {!!right && <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{right}</Text>}
      </View>
      {children}
    </Animated.View>
  );
}

function Results({ view, top }: { view: GlobalOpen; top: ReactNode }) {
  const t = useTheme();
  const tr = useT();
  const { byId } = useMembers();
  const { global, room, yesterday } = view;
  return (
    <Screen tabs={false}>
      {top}
      <Heading question={view.question} />
      <Section title={tr('globalQuestion.myRoom')} right={tr('globalQuestion.roomVoters', { voters: room.voters, members: room.members })}>
        <ResultBars result={bars(room.tally ?? [])} byId={byId} />
      </Section>
      <Section title={tr('globalQuestion.everywhere')} right={global.facts ? tr('globalQuestion.rooms', { count: global.rooms }) : undefined}>
        <Card style={{ padding: 16 }}>
          {global.facts ? <FactRows facts={global.facts.slice(0, 3)} /> : <NotEnough rooms={global.rooms} min={global.min_rooms} />}
        </Card>
      </Section>
      {yesterday && (
        <Section title={tr('globalQuestion.yesterday')}>
          <Card style={{ padding: 16, gap: 14 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 21, color: t.txt }}>{tidy(yesterday.question)}</Text>
            <Winner tally={yesterday.tally} byId={byId} />
            {yesterday.facts && <FactRows facts={yesterday.facts.slice(0, 2)} />}
          </Card>
        </Section>
      )}
    </Screen>
  );
}

/** Under the threshold: how many rooms answered so far, out of how many it takes. */
function NotEnough({ rooms, min }: { rooms: number; min: number }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Globe size={18} color={t.mut} />
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('globalQuestion.notEnough')}</Text>
      </View>
      <View style={{ height: 4, borderRadius: 2, backgroundColor: t.card2, overflow: 'hidden' }}>
        <View style={{ width: `${Math.min(100, (rooms / Math.max(min, 1)) * 100)}%`, height: 4, borderRadius: 2, backgroundColor: t.acc }} />
      </View>
      <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('globalQuestion.roomsSoFar', { count: rooms, min })}</Text>
    </View>
  );
}

/** Yesterday in my room: who came first (everyone tied for first). */
function Winner({ tally, byId }: { tally: GlobalCount[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const best = tally[0]?.count ?? 0;
  const first = tally.filter((c) => c.count === best && best > 0).map((c) => byId.get(c.target_id));
  if (!first.length) return <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>{tr('globalQuestion.noVotesYesterday')}</Text>;
  return (
    <View style={{ gap: 8 }}>
      {first.map((m, i) => (
        <View key={m?.id ?? `x${i}`} style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <Avatar member={m} size={32} />
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
            {m?.name ?? '?'}
          </Text>
          <Text style={{ fontFamily: t.font.display, fontSize: 18, color: t.txt }}>{best}</Text>
        </View>
      ))}
    </View>
  );
}
