import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  ArrowRight,
  Bell,
  BellOff,
  BellRing,
  Check,
  Eye,
  EyeOff,
  ChevronDown,
  Inbox,
  Moon,
  Pin,
  RotateCcw,
  Share2,
  SkipForward,
  Users,
} from 'lucide-react-native';
import { Redirect, router } from 'expo-router';
import { type ComponentRef, type ReactNode, useEffect, useRef, useState } from 'react';
import { enablePush, pushState, refreshPush, type PushState } from '@/lib/push';
import { Pressable, type ScrollView, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';

import { BackButton } from '@/components/auth-ui';
import { ChallengeButton, Challenges } from '@/components/challenge';
import { useSecretPicker } from '@/components/celebration';
import { ChatInput, ChatMessages } from '@/components/chat';
import { ChooseBallot, ChooseResult, myRealChoice } from '@/components/choose';
import { BIG_GROUP, gridColumns, MemberGrid, MemberTile } from '@/components/member-grid';
import { RankResult, ResultBars, ResultSummary } from '@/components/result-bars';
import { QueryState } from '@/components/query-state';
import { Screen } from '@/components/screen';
import { SecureCard } from '@/components/secure-card';
import { Tip } from '@/components/tip';
import { NextGate } from '@/components/next-gate';
import { NoQuestions, SoloInvite } from '@/components/vote-empty';
import { askConfirm, Avatar, AvatarStack, Button, Card, ConfirmModal, Dialog, Display, Logo, PressableScale, Tag, TAG_BG } from '@/components/ui';
import { api, ApiError, type Member, type Round } from '@/lib/api';
import { authError, useFeatures } from '@/lib/auth';
import { showToast } from '@/components/toast';
import { getLang, t as translate, useT } from '@/lib/i18n';
import { EASE, motion, usePop } from '@/lib/motion';
import { keys, useCurrentRound, useGroup, useInbox, useMembers, useRooms, useRound, useUnseenRound } from '@/lib/queries';
import { ordinal, tidy } from '@/lib/text';
import { useSession } from '@/lib/session';
import { setHideVote, useHideVote } from '@/lib/prefs';
import { confirmPublic, publicLink, questionMessage, resultMessage, shareText } from '@/lib/share';
import { useTheme } from '@/lib/theme';

function PushBell() {
  const t = useTheme();
  const tr = useT();
  const [state, setState] = useState<PushState>(pushState);
  useEffect(() => {
    refreshPush().catch(() => {});
  }, []);
  // Only shown while notifications can still be turned on.
  if (state === 'unsupported' || state === 'granted') return null;
  const denied = state === 'denied';
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={tr('vote.header.enablePush')}
      disabled={denied}
      onPress={() =>
        enablePush()
          .then(setState)
          .catch(() => setState(pushState()))
      }
      hitSlop={4}
      style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: denied ? t.card : t.acc }}>
      {denied ? <BellOff size={18} color={t.mut} /> : <Bell size={18} color={t.accTxt} />}
    </Pressable>
  );
}

/** Which room I'm in; tap to switch, create or join one. */
function RoomPill() {
  const t = useTheme();
  const tr = useT();
  const { data: group } = useGroup();
  const secret = useSecretPicker();
  if (!group) return null;
  return (
    <>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('vote.header.room', { name: group.name })}
        onPress={() => router.push('/rooms')}
        onLongPress={secret.onLongPress}
        hitSlop={6}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 4, alignSelf: 'flex-start', maxWidth: '100%' }}>
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
          {group.name}
        </Text>
        <ChevronDown size={14} color={t.mut} />
      </Pressable>
      {secret.picker}
    </>
  );
}

/** The in-app notifications, with a badge for what I have not seen. */
function InboxButton() {
  const t = useTheme();
  const tr = useT();
  const { data } = useInbox();
  const unread = data?.unread ?? 0;
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={unread ? tr('vote.header.inboxUnread', { count: unread }) : tr('vote.header.inbox')}
      onPress={() => router.push('/inbox')}
      hitSlop={4}
      style={[{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
      <Inbox size={18} color={t.txt} />
      {unread > 0 && (
        <View
          style={{
            position: 'absolute',
            top: -3,
            right: -3,
            minWidth: 18,
            height: 18,
            paddingHorizontal: 4,
            borderRadius: 9,
            backgroundColor: TAG_BG,
            alignItems: 'center',
            justifyContent: 'center',
          }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 10, color: '#fff' }}>{unread > 9 ? '9+' : unread}</Text>
        </View>
      )}
    </PressableScale>
  );
}

/** Opens the group page (members, and later invites and settings). */
function GroupButton() {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('vote.header.group')}
      onPress={() => router.push('/group')}
      hitSlop={4}
      style={[{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
      <Users size={18} color={t.txt} />
    </PressableScale>
  );
}

function Header() {
  return (
    <>
      <HeaderBar />
      <SecureCard />
    </>
  );
}

function HeaderBar() {
  const tr = useT();
  const { me } = useSession();
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
      <View style={{ gap: 4, flexShrink: 1 }}>
        <Logo />
        <RoomPill />
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <GroupButton />
        <InboxButton />
        <PushBell />
        <Pressable accessibilityRole="button" accessibilityLabel={tr('vote.header.profile')} onPress={() => router.push('/profile')}>
          <Avatar member={me ?? undefined} size={36} />
        </Pressable>
      </View>
    </View>
  );
}

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
  const roomId = group.data?.id;
  if (roomId != null && (group.data?.kind === 'party' || rooms?.find((r) => r.id === roomId)?.kind === 'party'))
    return <Redirect href={`/party/${roomId}`} />;

  // Recap first: a question I voted on finished while I was away (or with my vote).
  if (unseen.data) {
    const seen = () => api(`/rounds/${unseen.data!.id}/seen`, { method: 'POST' }).finally(() => qc.invalidateQueries({ queryKey: keys.unseen }));
    const gate = held === unseen.data.id && group.data ? <NextGate group={group.data} onLaunched={seen} /> : undefined;
    return <Result key={unseen.data.id} roundId={unseen.data.id} byId={byId} onNext={seen} gate={gate} />;
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
  if (round.my_vote != null) return <Waiting round={round} byId={byId} members={members} />;
  return <Ballot key={round.id} round={round} members={members} byId={byId} onVoted={() => qc.invalidateQueries({ queryKey: keys.unseen })} />;
}

// The last question shown to vote on, so it slides in only when a new one opens (not when Ballot turns into Waiting).
let lastQuestion: number | null = null;

function Question({ round, result }: { round: Round; result?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const [fresh] = useState(() => !result && round.id !== lastQuestion);
  useEffect(() => {
    if (!result) lastQuestion = round.id;
  }, [result, round.id]);
  const { byId } = useMembers();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const { moderation } = useFeatures();
  // The owner can take an abusive question of the room out of its result (long-press on the question).
  const retire =
    result && round.custom && moderation && group?.my_role === 'owner'
      ? async () => {
          if (!(await askConfirm(translate('vote.retireAsk'), translate('vote.retireMessage'), translate('vote.retireAction')))) return;
          await api(`/rounds/${round.id}/question`, { method: 'DELETE' }).catch((e) => showToast(authError(e)));
          qc.invalidateQueries({ queryKey: ['round'] });
          qc.invalidateQueries({ queryKey: ['history'] });
        }
      : undefined;
  const [copied, setCopied] = useState(false);
  const share = async () => {
    // The link opens a page whose preview is a picture of the card (the result's podium too, once out).
    if (!(await confirmPublic())) return;
    const link = await publicLink(round.id, !!result);
    const text = result ? resultMessage(round, byId) : questionMessage(round);
    const outcome = await shareText(link ? `${text.split('\n').slice(0, -1).join('\n')}\n${link}` : text);
    if (outcome === 'copied') {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    }
  };
  return (
    <Animated.View entering={fresh ? motion.question : undefined} style={{ gap: 14 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Tag label={`${tr(kindLabel(round, result))} · #${round.number}`} tone={result ? 'acc' : 'tag'} />
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          {result && <ChallengeButton round={round} />}
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr(copied ? 'vote.linkCopied' : result ? 'vote.shareResult' : 'vote.shareQuestion')}
            onPress={share}
            hitSlop={8}
            style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
            {copied ? <Check size={18} color={t.accInk} /> : <Share2 size={18} color={t.txt} />}
          </PressableScale>
        </View>
      </View>
      <View style={{ gap: 6 }}>
        <Pressable disabled={!retire} onLongPress={retire} delayLongPress={500} accessibilityHint={retire ? tr('vote.retireHint') : undefined}>
          <Display size={result ? 28 : 34}>{tidy(round.question)}</Display>
        </Pressable>
        {/* A deck question that came from someone's suggestion. */}
        {!!round.suggested_by && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
            {tr('community.credit', { name: round.suggested_by })}
          </Text>
        )}
      </View>
    </Animated.View>
  );
}

/** The tag over a question: its kind (a choose or ranking question says so even when it's ours), or "Résultat". */
const kindLabel = (round: Round, result?: boolean) =>
  result
    ? 'vote.kind.result'
    : round.kind === 'rank'
      ? 'vote.kind.rank'
      : round.kind === 'choose'
        ? 'vote.kind.choose'
        : round.custom
          ? 'vote.kind.custom'
          : 'vote.kind.pick';

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
    mutationFn: (ranking: number[]) => api('/round/current/vote', { method: 'POST', json: { ranking } }),
    onSuccess: onVoted,
    // Someone joined or left meanwhile: the room refreshed shows who to vote on now (the error is a toast).
    onError: () => qc.invalidateQueries({ queryKey: keys.group }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.current }),
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

function PickBallot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [pick, setPick] = useState<number | null>(null);
  const vote = useMutation({
    mutationFn: (target_id: number) => api('/round/current/vote', { method: 'POST', json: { target_id } }),
    // Remember the round right away: when my vote is the last one, the next question
    // replaces it before the refetch could show my vote, and the result must still show.
    onSuccess: onVoted,
    // Someone joined or left meanwhile: the room refreshed shows who to vote on now (the error is a toast).
    onError: () => qc.invalidateQueries({ queryKey: keys.group }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.current }),
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
    mutationFn: () => api('/round/current/nudge', { method: 'POST', json: { member_id: member.id } }),
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
    mutationFn: () => api(`/round/current/skip/${member.id}`, { method: mine ? 'DELETE' : 'PUT' }),
    onSettled: () => {
      setAsking(false);
      qc.invalidateQueries({ queryKey: keys.current });
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
  const pending = members.filter((m) => !round.voter_ids.includes(m.id));
  // The face tapped in the grid; closes by itself once they've voted.
  const [open, setOpen] = useState<number | null>(null);
  const opened = pending.find((m) => m.id === open);
  // With two people a majority is one person deciding alone: no skip vote then (nor when the owner turned it off).
  const group = useGroup();
  const canSkip = members.length >= 3 && group.data?.settings.skip_vote !== false;
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
      {/* Who's in, who we're waiting for. A card (not Card) that animates its height as people leave the list. */}
      <Animated.View layout={motion.layout} style={[{ backgroundColor: t.card, borderRadius: t.radius.card, padding: 16, gap: 14 }, t.shadow]}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <Text style={{ flex: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
            {tr('vote.waiting.voted', { count: done, done, total: round.member_count })}
          </Text>
          <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} size={26} ring={t.card} max={6} />
        </View>
        <Progress value={done / Math.max(round.member_count, 1)} />
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
    </Screen>
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

/** Back arrow (and pin) that replace the header when a result is opened from the history. */
function BackHeader({ onBack, round }: { onBack: () => void; round?: Round }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const pin = useMutation({
    mutationFn: (pinned: boolean) => api(`/rounds/${round!.id}/pin`, { method: pinned ? 'PUT' : 'DELETE' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.round(round!.id) });
      qc.invalidateQueries({ queryKey: keys.pinned });
    },
  });
  // Show the state being asked for right away; the server echo confirms it.
  const pinned = pin.isPending ? pin.variables : round?.pinned;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
      <BackButton onPress={onBack} />
      {round && (
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr(pinned ? 'vote.unpin' : 'vote.pin')}
          onPress={() => pin.mutate(!pinned)}
          disabled={pin.isPending}
          hitSlop={8}
          style={{
            width: 40,
            height: 40,
            borderRadius: 20,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: pinned ? t.accSoft : t.card,
          }}>
          <Pin size={18} color={pinned ? t.accInk : t.txt} fill={pinned ? t.accInk : 'none'} />
        </PressableScale>
      )}
    </View>
  );
}

export function Result({
  roundId,
  byId,
  onNext,
  onBack,
  gate,
}: {
  roundId: number;
  byId: Map<number, Member>;
  onNext?: () => void;
  onBack?: () => void;
  /** Shown instead of "Suivante" while a paced room waits for its next question. */
  gate?: ReactNode;
}) {
  const t = useTheme();
  const tr = useT();
  const roundQuery = useRound(roundId);
  const { data: round } = roundQuery;
  const scroll = useRef<ComponentRef<typeof ScrollView>>(null);
  // One scroll for the whole page (no box inside a box). Where the full result ends, and whether
  // it has scrolled out of sight: then its one-line summary stays pinned on top.
  const resultEnd = useRef(0);
  const [away, setAway] = useState(false);
  // Follow the newest message while the bottom is in view, and right after I write or send one.
  const follow = useRef(false);
  const toEnd = () => {
    follow.current = true;
    // After the keyboard has opened (and the page shrunk), not before.
    setTimeout(() => scroll.current?.scrollToEnd({ animated: true }), 300);
  };
  const header = onBack ? <BackHeader onBack={onBack} round={round ?? undefined} /> : <Header />;
  const next =
    gate ??
    (onNext && (
      <Button onPress={onNext}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('vote.next')}</Text>
        <ArrowRight size={20} color={t.accTxt} />
      </Button>
    ));
  // Not loaded (or failed): "Suivante" stays, so an unseen result never blocks the next question.
  if (!round?.result && !round?.choose)
    return (
      <Screen tabs={!onBack} footer={roundQuery.isLoading ? undefined : next}>
        {header}
        <QueryState query={roundQuery} />
      </Screen>
    );
  return (
    <Screen
      // Opened from the history, the screen has no tab bar to keep it clear of the iPhone home bar.
      tabs={!onBack}
      scroller={scroll}
      onScroll={({ contentOffset, layoutMeasurement, contentSize }) => {
        setAway(resultEnd.current > 0 && contentOffset.y > resultEnd.current - 24);
        follow.current = contentOffset.y + layoutMeasurement.height >= contentSize.height - 80;
      }}
      onContentSizeChange={() => follow.current && scroll.current?.scrollToEnd({ animated: true })}
      overlay={
        away && (
          <Animated.View entering={motion.drop} exiting={motion.exit} style={{ position: 'absolute', top: 8, left: 20, right: 20 }}>
            <ResultSummary
              result={round.result ?? []}
              ranking={round.ranking}
              choose={round.choose}
              byId={byId}
              onPress={() => {
                follow.current = false;
                scroll.current?.scrollTo({ y: 0, animated: true });
              }}
            />
          </Animated.View>
        )
      }
      footer={
        <>
          <ChatInput roundId={round.id} onFocus={toEnd} onSent={toEnd} />
          {next}
        </>
      }>
      {header}
      <View
        style={{ gap: 16 }}
        onLayout={({ nativeEvent: { layout } }) => {
          resultEnd.current = layout.y + layout.height;
        }}>
        <Question round={round} result />
        {round.kind === 'choose' && round.choose ? (
          <ChooseResult round={round} data={round.choose} byId={byId} />
        ) : round.ranking ? (
          <RankResult ranking={round.ranking} byId={byId} mine={round.my_ranking} />
        ) : (
          <ResultBars result={round.result ?? []} byId={byId} />
        )}
      </View>
      <Challenges roundId={round.id} />
      <ChatMessages roundId={round.id} />
    </Screen>
  );
}
