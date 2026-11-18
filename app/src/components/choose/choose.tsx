import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type ReactNode, useMemo, useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AboutMe } from '@/components/choose/about-me';
import { BestGuessers } from '@/components/choose/best-guessers';
import { ChooseReveal } from '@/components/choose/choose-reveal';
import { ChooseSummary } from '@/components/choose/choose-summary';
import { WireBoard } from '@/components/choose/wire-board';
import { AvatarStack } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Screen } from '@/components/ui/screen';
import { Tip } from '@/components/ui/tip';
import { groupKeys, type Member } from '@/lib/api/group';
import { type Choice, type ChooseResult as ChooseData, type Round, roundKeys, voteCurrent } from '@/lib/api/rounds';
import { useT } from '@/lib/i18n';
import { usePop } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

// "Qui choisirait quoi ?" (docs/CHOOSE.md): wire every member to an item (mine: my real choice, the others: my guesses),
// then the result person by person.

/**
 * The ballot of a choose question: everyone in a column on the left, the items on the right. Drag a person to an item
 * (or tap a person, then an item); I'm first, my wire is my real choice, the others' are my guesses.
 */
export function ChooseBallot({
  round,
  members,
  byId,
  onVoted,
  top,
}: {
  round: Round;
  members: Member[];
  byId: Map<number, Member>;
  onVoted: () => void;
  /** Header and question, drawn by the Vote tab. */
  top: ReactNode;
}) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me } = useSession();
  const items = round.items ?? [];
  // Me first: the one real choice, then everyone I guess for.
  const people = useMemo(() => {
    const mine = members.find((m) => m.id === me?.id);
    return mine ? [mine, ...members.filter((m) => m.id !== mine.id)] : members;
  }, [members, me?.id]);
  const [choices, setChoices] = useState<Record<number, number>>({});
  const done = people.filter((p) => choices[p.id] != null).length;
  const all = people.length > 0 && done === people.length;
  const vote = useMutation({
    meta: { quiet: true },
    mutationFn: (c: Choice[]) => voteCurrent({ choices: c }),
    onSuccess: onVoted,
    onSettled: () => qc.invalidateQueries({ queryKey: roundKeys.current }),
    // Someone joined or left meanwhile: refresh the room so the ballot shows who to wire now.
    onError: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });
  const send = () => all && vote.mutate(people.map((p) => ({ member_id: p.id, item: choices[p.id] })));
  const bump = usePop(all, 1.04, all);
  return (
    <Screen
      footer={
        <>
          {vote.isError && (
            <Text style={{ textAlign: 'center', fontFamily: t.font.semibold, fontSize: 13, color: t.tagInk }}>{tr('vote.choose.failed')}</Text>
          )}
          <Animated.View style={bump}>
            <Button testID="choose-vote" disabled={!all} loading={vote.isPending} onPress={send}>
              {all ? tr('vote.choose.submit') : tr('vote.choose.wired', { done, total: people.length })}
            </Button>
          </Animated.View>
        </>
      }>
      {top}
      {round.voter_ids.length > 0 && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} max={6} />
          <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
            {round.voter_ids.length}/{round.member_count}
          </Text>
        </View>
      )}
      <Tip id="choose" text={tr('vote.choose.tip')} />
      {items.length >= 2 && <WireBoard people={people} items={items} me={me?.id} choices={choices} onChange={setChoices} />}
    </Screen>
  );
}

/** My own real choice, once I voted (null: an older server, or not found). */
export function myRealChoice(round: Round, me: number | undefined): string | null {
  const c = round.my_choices?.find((x) => x.member_id === me);
  return c ? (round.items?.[c.item] ?? null) : null;
}

/** A finished choose question: one person at a time (or all at once), what they thought of me, who guessed best. */
export function ChooseResult({ round, data, byId }: { round: Round; data: ChooseData; byId: Map<number, Member> }) {
  const items = round.items ?? [];
  const { me } = useSession();
  const people = data.people.filter((p) => byId.has(p.member_id));
  const [all, setAll] = useState(false);
  const mine = people.find((p) => p.member_id === me?.id);
  return (
    <View style={{ gap: 20 }}>
      {all ? (
        <ChooseSummary people={people} items={items} byId={byId} onOne={() => setAll(false)} />
      ) : (
        <ChooseReveal people={people} items={items} byId={byId} onAll={() => setAll(true)} />
      )}
      {mine && mine.guesses.length > 0 && <AboutMe person={mine} items={items} byId={byId} />}
      <BestGuessers scores={data.scores} byId={byId} />
    </View>
  );
}
