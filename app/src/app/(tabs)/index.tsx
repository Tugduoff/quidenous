import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  ArrowLeft,
  ArrowRight,
  Bell,
  BellOff,
  BellRing,
  Check,
  Eye,
  EyeOff,
  ChevronDown,
  Inbox,
  Pin,
  Share2,
  SkipForward,
  Users,
} from 'lucide-react-native';
import { router } from 'expo-router';
import { type ReactNode, useEffect, useState } from 'react';
import { enablePush, pushState, refreshPush, type PushState } from '@/lib/push';
import { Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';

import { ChatInput, ChatMessages } from '@/components/chat';
import { RankResult, ResultBars } from '@/components/result-bars';
import { Screen } from '@/components/screen';
import { Avatar, AvatarStack, Button, Card, ConfirmModal, Display, Logo, PressableScale, Tag } from '@/components/ui';
import { api, type Member, type Round } from '@/lib/api';
import { EASE, motion, usePop } from '@/lib/motion';
import { keys, useCurrentRound, useGroup, useInbox, useMembers, useRound, useUnseenRound } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { setHideVote, useHideVote } from '@/lib/prefs';
import { publicLink, questionMessage, resultMessage, shareText } from '@/lib/share';
import { useTheme } from '@/lib/theme';

function PushBell() {
  const t = useTheme();
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
      accessibilityLabel="Activer les notifications"
      disabled={denied}
      onPress={() =>
        enablePush()
          .then(setState)
          .catch(() => setState(pushState()))
      }
      style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: denied ? t.card : t.acc }}>
      {denied ? <BellOff size={18} color={t.mut} /> : <Bell size={18} color={t.accTxt} />}
    </Pressable>
  );
}

/** Which room I'm in; tap to switch, create or join one. */
function RoomPill() {
  const t = useTheme();
  const { data: group } = useGroup();
  if (!group) return null;
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={`Salon ${group.name}, changer de salon`}
      onPress={() => router.push('/rooms')}
      hitSlop={6}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 4, alignSelf: 'flex-start' }}>
      <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut, maxWidth: 160 }}>
        {group.name}
      </Text>
      <ChevronDown size={14} color={t.mut} />
    </Pressable>
  );
}

/** The in-app notifications, with a badge for what I have not seen. */
function InboxButton() {
  const t = useTheme();
  const { data } = useInbox();
  const unread = data?.unread ?? 0;
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={unread ? `Notifications, ${unread} non lues` : 'Notifications'}
      onPress={() => router.push('/inbox')}
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
            backgroundColor: t.tag,
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
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel="Groupe"
      onPress={() => router.push('/group')}
      style={[{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
      <Users size={18} color={t.txt} />
    </PressableScale>
  );
}

function Header() {
  const { me } = useSession();
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}>
      <View style={{ gap: 4, flexShrink: 1 }}>
        <Logo />
        <RoomPill />
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <GroupButton />
        <InboxButton />
        <PushBell />
        <Pressable accessibilityRole="button" accessibilityLabel="Profil" onPress={() => router.push('/profile')}>
          <Avatar member={me ?? undefined} size={36} />
        </Pressable>
      </View>
    </View>
  );
}

export default function VoteScreen() {
  const t = useTheme();
  const { me } = useSession();
  const { byId, members } = useMembers();
  const current = useCurrentRound();
  const unseen = useUnseenRound();
  const qc = useQueryClient();
  const round = current.data;

  // Recap first: a question I voted on finished while I was away (or with my vote).
  if (unseen.data) {
    const seen = () => api(`/rounds/${unseen.data!.id}/seen`, { method: 'POST' }).finally(() => qc.invalidateQueries({ queryKey: keys.unseen }));
    return <Result key={unseen.data.id} roundId={unseen.data.id} byId={byId} onNext={seen} />;
  }
  if (!round) {
    return (
      <Screen>
        <Header />
        <Display>{current.isLoading ? '' : 'Plus de questions. Ajoutez-en !'}</Display>
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
  const [fresh] = useState(() => !result && round.id !== lastQuestion);
  useEffect(() => {
    if (!result) lastQuestion = round.id;
  }, [result, round.id]);
  const { byId } = useMembers();
  const [copied, setCopied] = useState(false);
  const share = async () => {
    // The link opens a page whose preview is a picture of the card (the result's podium too, once out).
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
        <Tag
          label={`${result ? 'Résultat' : round.kind === 'rank' ? 'Classement' : round.custom ? 'De nous' : 'Dilemme'} · #${round.number}`}
          tone={result ? 'acc' : 'tag'}
        />
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={result ? 'Partager le résultat' : 'Partager la question'}
          onPress={share}
          hitSlop={8}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 6,
            height: 32,
            paddingHorizontal: 12,
            borderRadius: t.radius.pill,
            backgroundColor: t.card,
          }}>
          {copied ? <Check size={15} color={t.accInk} /> : <Share2 size={15} color={t.txt} />}
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: copied ? t.accInk : t.txt }}>{copied ? 'Copié' : 'Partager'}</Text>
        </PressableScale>
      </View>
      <Display size={result ? 28 : 34}>{round.question}</Display>
    </Animated.View>
  );
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
  const qc = useQueryClient();
  const [order, setOrder] = useState<number[]>([]);
  const vote = useMutation({
    mutationFn: (ranking: number[]) => api('/round/current/vote', { method: 'POST', json: { ranking } }),
    onSuccess: onVoted,
    onSettled: () => qc.invalidateQueries({ queryKey: keys.current }),
  });
  const toggle = (id: number) => setOrder((o) => (o.includes(id) ? o.filter((x) => x !== id) : [...o, id]));
  const done = order.length === members.length;
  const next = order.length + 1;
  return (
    <Screen
      footer={
        <Springy trigger={done} when={done}>
          <Button disabled={!done} loading={vote.isPending} onPress={() => done && vote.mutate(order)}>
            {done ? 'Valider mon classement' : `Qui est ${next === 1 ? '1er' : `${next}e`} ?`}
          </Button>
        </Springy>
      }>
      <Header />
      <Question round={round} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} />
        <Text style={{ flex: 1, fontFamily: t.font.regular, color: t.mut }}>
          {round.voter_ids.length}/{round.member_count}
        </Text>
        {order.length > 0 && (
          <Pressable accessibilityRole="button" onPress={() => setOrder([])} hitSlop={8}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>Recommencer</Text>
          </Pressable>
        )}
      </View>
      <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>Touche chacun dans l'ordre, du premier au dernier.</Text>
      {/* Ranked people move up to their place, the rest slide down. */}
      <View style={{ gap: 10 }}>
        {[...order.map((id) => members.find((m) => m.id === id)!), ...members.filter((m) => !order.includes(m.id))].filter(Boolean).map((m) => {
          const place = order.indexOf(m.id) + 1;
          return (
            <Animated.View key={m.id} layout={motion.layout}>
              <PressableScale
                scale={0.98}
                accessibilityRole="button"
                accessibilityLabel={place ? `${m.name}, ${place}e. Retirer` : `Classer ${m.name}`}
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
    </Screen>
  );
}

function Ballot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  // Same component instance per round (keyed by id), so the kind never changes under it.
  if (round.kind === 'rank') return <RankBallot round={round} members={members} byId={byId} onVoted={onVoted} />;
  return <PickBallot round={round} members={members} byId={byId} onVoted={onVoted} />;
}

function PickBallot({ round, members, byId, onVoted }: { round: Round; members: Member[]; byId: Map<number, Member>; onVoted: () => void }) {
  const t = useTheme();
  const qc = useQueryClient();
  const [pick, setPick] = useState<number | null>(null);
  const vote = useMutation({
    mutationFn: (target_id: number) => api('/round/current/vote', { method: 'POST', json: { target_id } }),
    // Remember the round right away: when my vote is the last one, the next question
    // replaces it before the refetch could show my vote, and the result must still show.
    onSuccess: onVoted,
    onSettled: () => qc.invalidateQueries({ queryKey: keys.current }),
  });
  const picked = pick != null ? byId.get(pick) : undefined;
  return (
    <Screen
      footer={
        <Springy trigger={pick} when={pick != null}>
          <Button disabled={!picked} loading={vote.isPending} onPress={() => pick != null && vote.mutate(pick)}>
            {picked ? `Voter pour ${picked.name}` : `${round.voter_ids.length}/${round.member_count}`}
          </Button>
        </Springy>
      }>
      <Header />
      <Question round={round} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} />
        <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
          {round.voter_ids.length}/{round.member_count}
        </Text>
      </View>
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
    </Screen>
  );
}

/** Sends a "hurry up" push to someone who has not voted yet. Once per question. */
function NudgeButton({ member, roundId }: { member: Member; roundId: number }) {
  const t = useTheme();
  const [sent, setSent] = useState(false);
  useEffect(() => setSent(false), [roundId]);
  const nudge = useMutation({
    mutationFn: () => api('/round/current/nudge', { method: 'POST', json: { member_id: member.id } }),
    onSettled: () => setSent(true),
  });
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={`Relancer ${member.name}`}
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
        title={`Passer ${member.name} ?`}
        message="Si la majorité est d'accord, on n'attend plus son vote."
        confirmLabel="Passer"
        busy={skip.isPending}
        onConfirm={() => skip.mutate()}
        onCancel={() => setAsking(false)}
      />
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={mine ? `Ne plus passer ${member.name}` : `Passer ${member.name}`}
        disabled={skip.isPending}
        onPress={() => (mine ? skip.mutate() : setAsking(true))}
        hitSlop={6}
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 4,
          height: 32,
          paddingHorizontal: 10,
          borderRadius: t.radius.pill,
          backgroundColor: mine ? t.accSoft : t.card2,
        }}>
        <SkipForward size={14} color={mine ? t.accInk : t.mut} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: mine ? t.accInk : t.mut }}>
          {state ? `${state.count}/${round.skip_needed}` : 'Passer'}
        </Text>
      </PressableScale>
    </>
  );
}

function Waiting({ round, byId, members }: { round: Round; byId: Map<number, Member>; members: Member[] }) {
  const t = useTheme();
  const { me } = useSession();
  const target = round.my_vote != null ? byId.get(round.my_vote) : undefined;
  const hidden = useHideVote();
  const pending = members.filter((m) => !round.voter_ids.includes(m.id));
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
          <Text style={label}>{round.kind === 'rank' ? 'TON 1ER' : 'TON VOTE'}</Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 17, color: hidden ? t.mut : t.txt }}>
            {hidden ? 'Caché' : (target?.name ?? '?')}
          </Text>
        </View>
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={hidden ? 'Montrer mon vote' : 'Cacher mon vote'}
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
            {done} sur {round.member_count} ont voté
          </Text>
          <AvatarStack members={round.voter_ids.map((id) => byId.get(id))} size={26} ring={t.card} />
        </View>
        <Progress value={done / Math.max(round.member_count, 1)} />
        {pending.length > 0 && <Text style={label}>EN ATTENTE</Text>}
        {/* Someone who votes fades out of the list and the others close up. */}
        {pending.map((m) => (
          <Animated.View key={m.id} exiting={motion.exit} layout={motion.layout} style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
            <Avatar member={m} size={32} />
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
              {m.name}
            </Text>
            {round.skipped_ids.includes(m.id) && <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>passé</Text>}
            {m.id !== me?.id && !round.skipped_ids.includes(m.id) && <SkipButton member={m} round={round} />}
            {m.id !== me?.id && <NudgeButton member={m} roundId={round.id} />}
          </Animated.View>
        ))}
      </Animated.View>
    </Screen>
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
  const qc = useQueryClient();
  const pin = useMutation({
    mutationFn: (pinned: boolean) => api(`/rounds/${round!.id}/pin`, { method: pinned ? 'PUT' : 'DELETE' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.round(round!.id) });
      qc.invalidateQueries({ queryKey: keys.history });
    },
  });
  // Show the state being asked for right away; the server echo confirms it.
  const pinned = pin.isPending ? pin.variables : round?.pinned;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={onBack}
        hitSlop={8}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 6, height: 36 }}>
        <ArrowLeft size={22} color={t.txt} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Historique</Text>
      </Pressable>
      {round && (
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={pinned ? 'Désépingler' : 'Épingler'}
          onPress={() => pin.mutate(!pinned)}
          disabled={pin.isPending}
          hitSlop={8}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 6,
            height: 36,
            paddingHorizontal: 14,
            borderRadius: t.radius.pill,
            backgroundColor: pinned ? t.accSoft : t.card,
          }}>
          <Pin size={16} color={pinned ? t.accInk : t.txt} fill={pinned ? t.accInk : 'none'} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: pinned ? t.accInk : t.txt }}>{pinned ? 'Épinglé' : 'Épingler'}</Text>
        </PressableScale>
      )}
    </View>
  );
}

export function Result({ roundId, byId, onNext, onBack }: { roundId: number; byId: Map<number, Member>; onNext?: () => void; onBack?: () => void }) {
  const t = useTheme();
  const { data: round } = useRound(roundId);
  const header = onBack ? <BackHeader onBack={onBack} round={round ?? undefined} /> : <Header />;
  if (!round?.result) return <Screen tabs={!onBack}>{header}</Screen>;
  return (
    <Screen
      // Opened from the history, the screen has no tab bar to keep it clear of the iPhone home bar.
      tabs={!onBack}
      footer={
        <>
          <ChatInput roundId={round.id} />
          {onNext && (
            <Button onPress={onNext}>
              <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>Suivante</Text>
              <ArrowRight size={20} color={t.accTxt} />
            </Button>
          )}
        </>
      }>
      {header}
      <Question round={round} result />
      {round.ranking ? <RankResult ranking={round.ranking} byId={byId} mine={round.my_ranking} /> : <ResultBars result={round.result} byId={byId} />}
      <ChatMessages roundId={round.id} />
    </Screen>
  );
}
