import { useMutation, useQueryClient } from '@tanstack/react-query';
import { BellRing, Check, Eye, EyeOff, Moon, RotateCcw, SkipForward } from 'lucide-react-native';
import { Redirect } from 'expo-router';
import { type ReactNode, useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';
import { ChooseBallot, myRealChoice } from '@/components/choose/choose';
import { SaidRound } from '@/components/said/said';
import { BIG_GROUP, gridColumns, MemberGrid, MemberTile } from '@/components/vote/member-grid';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { StreakBadge } from '@/components/streaks/streaks';
import { Tip } from '@/components/ui/tip';
import { BetRow } from '@/components/tripot/tripot';
import { NextGate } from '@/components/vote/next-gate';
import { NoQuestions, SoloInvite } from '@/components/vote/vote-empty';
import { Mascot } from '@/components/mascot/mascot';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { EASE, motion, usePop } from '@/lib/motion';
import { ordinal } from '@/lib/utils/text';
import { useSession } from '@/lib/session';
import { setHideVote, useHideVote } from '@/lib/device/prefs';
import { useTheme } from '@/lib/theme';
import { useLandOnToday } from '@/lib/hooks/use-today-tab';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar, AvatarStack } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { ConfirmModal, Dialog } from '@/components/ui/dialog';
import { type Member, useGroup, useMembers, groupKeys } from '@/lib/api/group';
import { type Round, useCurrentRound, useUnseenRound, roundKeys, markRoundSeen, voteCurrent, nudgeMember, setSkip, type SaidView } from '@/lib/api/rounds';
import { useRooms } from '@/lib/api/rooms';
import { Result } from '@/components/result/result';
import { Question } from '@/components/vote/question';
import { Header } from '@/components/vote/header';

export default function VoteScreen() {
  const { byId, members } = useMembers();
  const current = useCurrentRound();
  const unseen = useUnseenRound();
  const qc = useQueryClient();
  const group = useGroup();
  const round = current.data;
  // Paced room: the last result stays up until the owner launches the next question or its delay is over.
  const pace = group.data?.pace;
  const held = !round && pace?.waiting && pace.round_id ? pace.round_id : null;
  // A party has its own screen (the room list says so too, in case the room's own answer doesn't).
  const { data: rooms } = useRooms();
  // A plain start with two rooms or more opens on « Aujourd'hui » (lib/today.ts): straight there, nothing drawn here.
  const land = useLandOnToday();
  if (land) return <Redirect href="/today" />;
  const roomId = group.data?.id;
  if (roomId != null && (group.data?.kind === 'party' || rooms?.find((r) => r.id === roomId)?.kind === 'party'))
    return <Redirect href={`/party/${roomId}`} />;

  // Recap first: a question I voted on finished while I was away (or with my vote).
  if (unseen.data) {
    const seenUpTo = (id: number) => markRoundSeen(id).finally(() => qc.invalidateQueries({ queryKey: roundKeys.unseen }));
    const unseenId = unseen.data.id;
    const seen = () => seenUpTo(unseenId);
    const gate = held === unseen.data.id && group.data ? <NextGate group={group.data} onLaunched={seen} /> : undefined;
    // Back after days away: skip the pile of results straight to today's question (`seen_round` only moves forward).
    const { unseen_count: count = 1, unseen_last_id: last } = unseen.data;
    const skip = !gate && count >= 2 && last != null ? { count, onPress: () => seenUpTo(last) } : undefined;
    return <Result key={unseen.data.id} roundId={unseen.data.id} byId={byId} onNext={seen} gate={gate} skip={skip} />;
  }
  // Alone in the room: invite first (an older room may still have a question open, nobody else would vote on it).
  // Someone joining refreshes the room (live event), and this turns into the question by itself.
  if (group.data && group.data.members.length < 2) {
    return (
      <Screen>
        <Header />
        <SoloInvite />
      </Screen>
    );
  }
  if (held && group.data) return <Result key={held} roundId={held} byId={byId} gate={<NextGate group={group.data} />} />;
  if (!round) {
    return (
      <Screen>
        <Header />
        {/* Can't reach the server: say so (and retry), never "no questions". */}
        {current.isError || group.isError ? (
          <QueryState query={current.isError ? current : group} />
        ) : (
          !current.isLoading && !group.isLoading && <NoQuestions />
        )}
      </Screen>
    );
  }
  // A « Qui a dit ça ? » question has its own screens: write, guess, then wait (docs/SAID.md).
  if (round.kind === 'said' && round.said)
    return <SaidBallot key={round.id} round={round} said={round.said} members={members} byId={byId} onVoted={() => qc.invalidateQueries({ queryKey: roundKeys.unseen })} />;
  if (round.my_vote != null) return <Waiting round={round} byId={byId} members={members} />;
  return <Ballot key={round.id} round={round} members={members} byId={byId} onVoted={() => qc.invalidateQueries({ queryKey: roundKeys.unseen })} />;
}

/** First-run hint on the ballot: how a question ends. */
function VoteTip() {
  const tr = useT();
  return <Tip id="vote" text={tr('vote.tip')} />;
}

/** The vote button bumps when it becomes usable (and each time the pick changes), so the next step is obvious. */
function Springy({ trigger, when, children }: { trigger: unknown; when: boolean; children: ReactNode }) {
  return <Animated.View style={usePop(trigger, 1.04, when)}>{children}</Animated.View>;
}

/** Bumps its child when `on` turns true: the face just picked, the place number just given. */
function Pop({ on, to = 1.15, children }: { on: boolean; to?: number; children: ReactNode }) {
  return <Animated.View style={usePop(on, to, on)}>{children}</Animated.View>;
}

/** Ranking question: tap everyone in order, first to last. Tap a ranked person again to take them out. */
function RankBallot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
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

function Ballot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  // Same component instance per round (keyed by id), so the kind never changes under it.
  if (round.kind === 'rank') return <RankBallot round={round} members={members} byId={byId} onVoted={onVoted} />;
  // A choose question without its items (shouldn't happen) falls back to a plain pick.
  if (round.kind === 'choose' && (round.items?.length ?? 0) >= 2)
    return (
      <ChooseBallot
        round={round}
        members={members}
        byId={byId}
        onVoted={onVoted}
        top={
          <>
            <Header />
            <Question round={round} />
          </>
        }
      />
    );
  return <PickBallot round={round} members={members} byId={byId} onVoted={onVoted} />;
}

function SaidBallot({ round, said, members, byId, onVoted }: { round: Round; said: SaidView; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  const { me } = useSession();
  return (
    <SaidRound
      round={round}
      said={said}
      me={me?.id}
      byId={byId}
      onVoted={onVoted}
      top={
        <>
          <Header />
          <Question round={round} />
        </>
      }
      // While writing, "done" is having written; then, having guessed.
      waitList={(done, title) => <WhoIsIn round={round} members={members} byId={byId} done={done} title={title} />}
    />
  );
}

function PickBallot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [pick, setPick] = useState<number | null>(null);
  const vote = useMutation({
    mutationFn: (target_id: number) => voteCurrent({ target_id }),
    // Remember the round right away: when my vote is the last one, the next question
    // replaces it before the refetch could show my vote, and the result must still show.
    onSuccess: onVoted,
    // Someone joined or left meanwhile: the room refreshed shows who to vote on now (the error is a toast).
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
      {/* Nobody yet: the button below already says 0/N. */}
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

/** Sends a "hurry up" push to someone who has not voted yet. Once per question. */
function NudgeButton({ member, roundId }: { member: Member; roundId: number }) {
  const t = useTheme();
  const tr = useT();
  const [sent, setSent] = useState(false);
  useEffect(() => setSent(false), [roundId]);
  const nudge = useMutation({
    mutationFn: () => nudgeMember(member.id),
    // Nudged already (another phone of mine, a lost answer): same as sent. Any other failure: a toast, the bell stays.
    meta: { quiet: (e) => e instanceof ApiError && e.code === 'already_nudged' },
    onSuccess: () => setSent(true),
    onError: (e) => e instanceof ApiError && e.code === 'already_nudged' && setSent(true),
  });
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('vote.nudge', { name: member.name })}
      disabled={sent || nudge.isPending}
      onPress={() => nudge.mutate()}
      style={{
        width: 36,
        height: 36,
        borderRadius: 18,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: sent ? t.card2 : t.accSoft,
      }}>
      {sent ? <Check size={18} color={t.mut} /> : <BellRing size={18} color={t.accInk} />}
    </PressableScale>
  );
}

/** "Let's not wait": agree to close the question without them. Shows how many already agree. */
function SkipButton({ member, round }: { member: Member; round: Round }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const state = round.skips.find((s) => s.target_id === member.id);
  const mine = !!state?.mine;
  // Asking to skip someone takes a confirmation (it sits right next to the bell); taking it back doesn't.
  const [asking, setAsking] = useState(false);
  const skip = useMutation({
    mutationFn: () => setSkip(member.id, !mine),
    onSettled: () => {
      setAsking(false);
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
  });
  return (
    <>
      <ConfirmModal
        visible={asking}
        title={tr('vote.skip.title', { name: member.name })}
        message={tr('vote.skip.message')}
        confirmLabel={tr('vote.skip.confirm')}
        busy={skip.isPending}
        onConfirm={() => skip.mutate()}
        onCancel={() => setAsking(false)}
      />
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr(mine ? 'vote.skip.undo' : 'vote.skip.label', { name: member.name })}
        disabled={skip.isPending}
        onPress={() => (mine ? skip.mutate() : setAsking(true))}
        hitSlop={6}
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 4,
          minWidth: 36,
          height: 36,
          paddingHorizontal: state ? 10 : 0,
          borderRadius: 18,
          backgroundColor: mine ? t.accSoft : t.card2,
        }}>
        <SkipForward size={16} color={mine ? t.accInk : t.mut} />
        {state && (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: mine ? t.accInk : t.mut }}>
            {state.count}/{round.skip_needed}
          </Text>
        )}
      </PressableScale>
    </>
  );
}

function Waiting({ round, byId, members }: { round: Round; byId: Map<number, Member>; members: Member[] }) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  // A choose question: my own real choice (my vote's target is me).
  const choice = round.kind === 'choose' ? myRealChoice(round, me?.id) : null;
  const target = round.my_vote != null ? byId.get(round.my_vote) : undefined;
  const hidden = useHideVote();
  const done = round.voter_ids.length;
  const label = { fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.6, color: t.mut } as const;
  return (
    <Screen>
      <Header />
      <Question round={round} />
      {/* My vote: who I picked (or hidden), one switch to hide it when friends can see my screen. */}
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14 }}>
        {hidden ? (
          <View style={{ width: 44, height: 44, borderRadius: 22, backgroundColor: t.card2, alignItems: 'center', justifyContent: 'center' }}>
            <EyeOff size={20} color={t.mut} />
          </View>
        ) : (
          <Avatar member={target} size={44} />
        )}
        <View style={{ flex: 1, gap: 2 }}>
          <Text style={label}>
            {tr(round.kind === 'rank' ? 'vote.waiting.yourFirst' : round.kind === 'choose' ? 'vote.waiting.yourChoice' : 'vote.waiting.yourVote')}
          </Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 17, color: hidden ? t.mut : t.txt }}>
            {hidden ? tr('vote.waiting.hidden') : round.kind === 'choose' ? (choice ?? '?') : (target?.name ?? '?')}
          </Text>
        </View>
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr(hidden ? 'vote.waiting.show' : 'vote.waiting.hide')}
          onPress={() => setHideVote(!hidden)}
          hitSlop={8}
          style={{ width: 36, height: 36, borderRadius: 18, backgroundColor: t.card2, alignItems: 'center', justifyContent: 'center' }}>
          {hidden ? <Eye size={18} color={t.txt} /> : <EyeOff size={18} color={t.txt} />}
        </PressableScale>
      </Card>
      <BetRow round={round} members={members} byId={byId} />
      <WhoIsIn
        round={round}
        members={members}
        byId={byId}
        done={round.voter_ids}
        title={tr('vote.waiting.voted', { count: done, done, total: round.member_count })}
      />
    </Screen>
  );
}

/**
 * Who's in, who we're waiting for (`done`: who voted, or for a said question who wrote). A card (not Card) that animates
 * its height as people leave the list; each pending person with the skip and the bell.
 */
function WhoIsIn({
  round,
  members,
  byId,
  done,
  title,
}: {
  round: Round;
  members: Member[];
  byId: Map<number, Member>;
  done: number[];
  title: string;
}) {
  const t = useTheme();
  const tr = useT();
  const pending = members.filter((m) => !done.includes(m.id));
  // The face tapped in the grid; closes by itself once they've voted.
  const [open, setOpen] = useState<number | null>(null);
  const opened = pending.find((m) => m.id === open);
  // With two people a majority is one person deciding alone: no skip vote then (nor when the owner turned it off).
  const group = useGroup();
  const canSkip = members.length >= 3 && group.data?.settings.skip_vote !== false;
  const label = { fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.6, color: t.mut } as const;
  return (
    <>
      {/* Who's in, who we're waiting for. A card (not Card) that animates its height as people leave the list. */}
      <Animated.View layout={motion.layout} style={[{ backgroundColor: t.card, borderRadius: t.radius.card, padding: 16, gap: 14 }, t.shadow]}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          {/* Roupi dozes while the others take their time (not when there's a whole grid of faces to show). */}
          {pending.length > 0 && pending.length <= BIG_WAIT && (
            <Mascot who="roupi" mood="sleepy" size={56} style={{ marginVertical: -8, marginLeft: -6 }} />
          )}
          <Text style={{ flex: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{title}</Text>
          <AvatarStack members={done.map((id) => byId.get(id))} size={26} ring={t.card} max={6} />
        </View>
        <Progress value={done.length / Math.max(round.member_count, 1)} />
        {pending.length > 0 && <Text style={label}>{tr('vote.waiting.pending')}</Text>}
        {/* Many still to vote: a grid of faces; tap one for the bell and the skip. */}
        {pending.length > BIG_WAIT ? (
          <MemberGrid>
            {pending.map((m) => {
              const skipped = round.skipped_ids.includes(m.id);
              const away = !!round.away_ids?.includes(m.id);
              const asked = round.skips.some((s) => s.target_id === m.id);
              return (
                <Animated.View key={m.id} exiting={motion.exit} layout={motion.layout} style={{ width: '20%' }}>
                  <MemberTile
                    member={m}
                    cols={1}
                    plain
                    dim={skipped || away}
                    badge={skipped || asked ? <SkipForward size={12} color={t.mut} /> : away ? <Moon size={12} color={t.mut} /> : undefined}
                    label={
                      skipped ? tr('vote.waiting.skippedLabel', { name: m.name }) : away ? tr('vote.waiting.awayLabel', { name: m.name }) : m.name
                    }
                    onPress={() => setOpen(m.id)}
                  />
                </Animated.View>
              );
            })}
          </MemberGrid>
        ) : (
          /* Someone who votes fades out of the list and the others close up. */
          pending.map((m) => <PendingRow key={m.id} member={m} round={round} canSkip={canSkip} />)
        )}
      </Animated.View>
      <Dialog visible={!!opened} onClose={() => setOpen(null)}>
        {opened && <PendingRow member={opened} round={round} canSkip={canSkip} big />}
      </Dialog>
    </>
  );
}

/** Up to this many people still to vote, one row each; more, a grid of faces. */
const BIG_WAIT = 5;

/** Someone we're waiting for: name, "passé" once skipped, the skip and the bell. */
function PendingRow({ member: m, round, canSkip, big }: { member: Member; round: Round; canSkip: boolean; big?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const skipped = round.skipped_ids.includes(m.id);
  // Away (missed the last questions in a row): not waited for, no skip needed.
  const away = !skipped && !!round.away_ids?.includes(m.id);
  return (
    <Animated.View exiting={motion.exit} layout={motion.layout} style={{ flexDirection: 'row', alignItems: 'center', gap: big ? 12 : 10 }}>
      <Avatar member={m} size={big ? 44 : 32} />
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: big ? t.font.bold : t.font.semibold, fontSize: big ? 17 : 15, color: t.txt }}>
        {m.name}
      </Text>
      <StreakBadge id={m.id} />
      {(skipped || away) && (
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr(skipped ? 'vote.waiting.skipped' : 'vote.waiting.away')}</Text>
      )}
      {canSkip && m.id !== me?.id && !skipped && !away && <SkipButton member={m} round={round} />}
      {m.id !== me?.id && <NudgeButton member={m} roundId={round.id} />}
    </Animated.View>
  );
}

/** Share of the group that has voted (0 to 1): fills up from empty, then grows with each new vote. */
function Progress({ value }: { value: number }) {
  const t = useTheme();
  const fill = useSharedValue(0);
  useEffect(() => {
    fill.value = withTiming(value, { duration: 450, easing: EASE });
  }, [value, fill]);
  const width = useAnimatedStyle(() => ({ width: `${fill.value * 100}%` }));
  return (
    <View style={{ height: 6, borderRadius: 3, backgroundColor: t.card2, overflow: 'hidden' }}>
      <Animated.View style={[{ height: 6, borderRadius: 3, backgroundColor: t.acc }, width]} />
    </View>
  );
}
