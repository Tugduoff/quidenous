import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import {
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  Ellipsis,
  ExternalLink,
  Hourglass,
  LogOut,
  Monitor,
  Share2,
  Square,
  UserPlus,
} from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { ActivityIndicator, Linking, Platform, Pressable, Text, useWindowDimensions, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { copyText, ErrorText, SoftButton } from '@/components/auth-ui';
import { BIG_GROUP, gridColumns, MemberGrid, MemberTile } from '@/components/member-grid';
import { Countdown, JoinQr, NextButton, PlayerChips, Reactions, VotedLine, WinnerReveal } from '@/components/party';
import { ResultBars } from '@/components/result-bars';
import { Screen } from '@/components/screen';
import { askConfirm, Avatar, Button, Card, CodeText, Dialog, DialogTitle, Display, PressableScale, Segmented, Tag } from '@/components/ui';
import { api, ApiError, type HistoryItem, type Member, setApiRoom } from '@/lib/api';
import { dateFormat, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useMembers } from '@/lib/queries';
import {
  type Party,
  partyApi,
  partyError,
  partyJoinLink,
  partyScreenLink,
  type PartyRound,
  tallies,
  tidy,
  TIMERS,
  useCountdown,
  useParty,
  useRefetchAtZero,
  useSetParty,
} from '@/lib/party';
import { useRoomGone, useSwitchRoom } from '@/lib/rooms';
import { useSession } from '@/lib/session';
import { shareText } from '@/lib/share';
import { useTheme } from '@/lib/theme';

/** `/party/[id]`: the whole party on one screen, following its phase (lobby, question, result, memory). */
export default function PartyScreen() {
  const t = useTheme();
  const tr = useT();
  const { id: raw } = useLocalSearchParams<{ id: string }>();
  const id = Number(raw);
  const qc = useQueryClient();
  const { room, setRoom } = useSession();
  const party = useParty(id);
  const p = party.data;
  const loaded = !!p;

  // This party becomes the current room: the live socket follows it, and its memory reads its history.
  useEffect(() => {
    if (!loaded || room === id) return;
    setApiRoom(id);
    qc.removeQueries({ predicate: (q) => q.queryKey[0] !== 'party' });
    setRoom(id);
  }, [loaded, room, id, qc, setRoom]);

  // Turned into a normal room ("Continuer en salon"): the room's usual screens take over.
  const gone = party.error instanceof ApiError && [404, 409].includes(party.error.status);
  useEffect(() => {
    if (p?.kind === 'room' || (loaded && gone)) router.replace('/');
  }, [p?.kind, loaded, gone]);

  if (!p)
    return (
      <Screen tabs={false}>
        <BackToRooms />
        {party.error ? (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.mut }}>{partyError(party.error)}</Text>
        ) : (
          <ActivityIndicator color={t.mut} style={{ marginTop: 40 }} />
        )}
      </Screen>
    );
  if (p.phase === 'lobby') return <Lobby party={p} />;
  if (p.phase === 'voting' && p.round) return <Voting key={p.round.id} party={p} round={p.round} />;
  if (p.phase === 'reveal' && p.round) return <Reveal key={p.round.id} party={p} round={p.round} />;
  if (p.phase === 'over') return <Memory party={p} />;
  return (
    <Screen tabs={false}>
      <Header party={p} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.mut }}>{tr('common.failed')}</Text>
    </Screen>
  );
}

const byIdOf = (p: Party) => new Map<number, Member>(p.players.map((m) => [m.id, m]));

/** The players, plus people who left since (their votes and wins keep a face). */
function usePeople(party: Party) {
  const { byId } = useMembers();
  const all = new Map<number, Member>(byId);
  for (const m of party.players) all.set(m.id, m);
  return all;
}
const isOwner = (p: Party) => p.me?.role === 'owner';

function BackToRooms() {
  const t = useTheme();
  const tr = useT();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={tr('common.back')}
      onPress={() => router.replace('/rooms')}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
      <ChevronLeft size={22} color={t.txt} />
    </Pressable>
  );
}

/** Back to the rooms, the party's name and when it ends, and its menu. */
function Header({ party }: { party: Party }) {
  const t = useTheme();
  const tr = useT();
  const [menu, setMenu] = useState(false);
  const sub =
    party.phase === 'over'
      ? tr('party.over')
      : tr('party.until', { time: dateFormat({ hour: '2-digit', minute: '2-digit' }).format(new Date(party.ends_at)) });
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
      <BackToRooms />
      <View style={{ flex: 1, minWidth: 0 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
          {party.name || tr('party.name')}
        </Text>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {sub}
        </Text>
      </View>
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr('party.menu.a11y')}
        onPress={() => setMenu(true)}
        style={[{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
        <Ellipsis size={20} color={t.txt} />
      </PressableScale>
      <Menu party={party} visible={menu} onClose={() => setMenu(false)} />
    </View>
  );
}

/** Owner: time per question, end the party. Everyone: leave. */
function Menu({ party, visible, onClose }: { party: Party; visible: boolean; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  const setParty = useSetParty(party.id);
  const roomGone = useRoomGone();
  const [error, setError] = useState<string | null>(null);
  const owner = isOwner(party);
  const live = party.phase !== 'over';
  const timer = useMutation({
    mutationFn: (s: number) => partyApi.update(party.id, { timer: s }),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => setError(partyError(e)),
  });
  const end = async () => {
    onClose();
    if (!(await askConfirm(tr('party.menu.endAsk'), tr('party.menu.endMessage'), tr('party.menu.endAction')))) return;
    partyApi.end(party.id).then(setParty, () => {});
  };
  const leave = async () => {
    onClose();
    if (!(await askConfirm(tr('party.menu.leaveAsk'), tr('party.menu.leaveMessage'), tr('common.leave')))) return;
    await partyApi.leave(party.id).catch(() => {});
    await roomGone();
  };
  const row = (Icon: typeof LogOut, label: string, onPress: () => void, danger?: boolean) => (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 12, height: 48, paddingHorizontal: 14, borderRadius: 14, backgroundColor: t.card2 }}>
      <Icon size={18} color={danger ? t.tag : t.txt} />
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: danger ? t.tag : t.txt }}>{label}</Text>
    </Pressable>
  );
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={party.name || tr('party.name')} />
      {owner && live && (
        <View style={{ gap: 8 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('party.menu.timer')}</Text>
          <Segmented
            label={tr('party.menu.timer')}
            value={String(party.timer)}
            onChange={(v) => timer.mutate(Number(v))}
            options={TIMERS.map((s) => ({ id: String(s), name: tr('party.create.seconds', { count: s }) }))}
          />
        </View>
      )}
      <ErrorText>{error}</ErrorText>
      {owner && live && row(Square, tr('party.menu.end'), end, true)}
      {row(LogOut, tr('party.menu.leave'), leave)}
    </Dialog>
  );
}

/** Invite, wait for everyone, and (owner) start. */
function Lobby({ party }: { party: Party }) {
  const t = useTheme();
  const tr = useT();
  const { width } = useWindowDimensions();
  const setParty = useSetParty(party.id);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<'link' | 'code' | null>(null);
  const flash = (what: 'link' | 'code') => {
    setCopied(what);
    setTimeout(() => setCopied(null), 1800);
  };
  const start = useMutation({
    mutationFn: () => partyApi.start(party.id),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => setError(partyError(e)),
  });
  const owner = isOwner(party);
  const enough = party.players.length >= 2;
  const qr = Math.min(260, Math.max(160, Math.min(width, 520) - 120));
  const share = async () => {
    const r = await shareText(tr('party.lobby.message', { name: party.name || tr('party.name'), link: partyJoinLink(party.code) }));
    if (r === 'copied') flash('link');
  };
  const round = { width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 } as const;
  return (
    <Screen
      tabs={false}
      footer={
        <>
          <ErrorText>{error}</ErrorText>
          {owner ? (
            <Button disabled={!enough} loading={start.isPending} onPress={() => start.mutate()}>
              {tr('party.lobby.start')}
            </Button>
          ) : (
            <Text style={{ textAlign: 'center', paddingVertical: 12, fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>
              {tr('party.lobby.waiting')}
            </Text>
          )}
        </>
      }>
      <Header party={party} />
      <Card style={{ padding: 18, gap: 14 }}>
        <JoinQr code={party.code} size={qr} />
        <CodeText code={party.code} size={48} center />
        <View style={{ flexDirection: 'row', gap: 10 }}>
          <PressableScale
            accessibilityRole="button"
            onPress={share}
            style={{
              flex: 1,
              height: 44,
              borderRadius: t.radius.pill,
              flexDirection: 'row',
              gap: 8,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.acc,
            }}>
            <Share2 size={18} color={t.accTxt} />
            <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>
              {copied === 'link' ? tr('party.lobby.linkCopied') : tr('party.lobby.share')}
            </Text>
          </PressableScale>
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={copied === 'code' ? tr('room.invite.codeCopied') : tr('room.invite.copyCode')}
            onPress={async () => (await copyText(party.code)) && Platform.OS === 'web' && flash('code')}
            style={round}>
            {copied === 'code' ? <Check size={18} color={t.accInk} /> : <Copy size={18} color={t.txt} />}
          </PressableScale>
        </View>
      </Card>

      <View style={{ gap: 10 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, marginLeft: 4 }}>
          <UserPlus size={14} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
            {tr('party.lobby.players', { count: party.players.length })}
          </Text>
        </View>
        <PlayerChips players={party.players} />
      </View>

      <ScreenLink code={party.code} />
    </Screen>
  );
}

/** The big-screen view: open it here, or send its link to the computer on the TV. */
function ScreenLink({ code }: { code: string }) {
  const t = useTheme();
  const tr = useT();
  const [copied, setCopied] = useState(false);
  const url = partyScreenLink(code);
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, paddingVertical: 8, paddingLeft: 16, paddingRight: 8 }}>
      <Monitor size={18} color={t.txt} />
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
        {tr('party.lobby.screen')}
      </Text>
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={copied ? tr('party.lobby.linkCopied') : tr('party.lobby.screenLink')}
        onPress={async () => {
          if ((await shareText(url)) !== 'copied') return;
          setCopied(true);
          setTimeout(() => setCopied(false), 1800);
        }}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        {copied ? <Check size={18} color={t.accInk} /> : <Share2 size={18} color={t.txt} />}
      </PressableScale>
      <PressableScale
        accessibilityRole="link"
        accessibilityLabel={tr('party.lobby.openScreen')}
        onPress={() => Linking.openURL(url).catch(() => {})}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        <ExternalLink size={18} color={t.txt} />
      </PressableScale>
    </Card>
  );
}

/** The question number and text. */
function Question({ round, size }: { round: PartyRound; size: number }) {
  return (
    <Animated.View entering={motion.question} style={{ gap: 12 }}>
      <Tag label={`#${round.number}`} />
      <Display size={size}>{tidy(round.question)}</Display>
    </Animated.View>
  );
}

/** The open question: countdown, pick someone, then who has voted. Someone who arrived after it opened watches. */
function Voting({ party, round }: { party: Party; round: PartyRound }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const ms = useCountdown(round.deadline_at, party.clock);
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: ['party', party.id] }));
  const [pick, setPick] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const byId = byIdOf(party);
  const me = party.me?.member_id;
  const late = me == null || !round.eligible.includes(me);
  const voted = round.my_vote != null || (me != null && round.voted.includes(me));
  const vote = useMutation({
    mutationFn: (target: number) => partyApi.vote(party.id, target),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => {
      setError(partyError(e));
      qc.invalidateQueries({ queryKey: ['party', party.id] });
    },
  });
  const choices = round.eligible.map((pid) => byId.get(pid)).filter((m): m is Member => !!m);
  const picked = pick != null ? byId.get(pick) : undefined;
  const mine = round.my_vote != null ? byId.get(round.my_vote) : undefined;
  return (
    <Screen
      tabs={false}
      footer={
        !late &&
        !voted && (
          <>
            <ErrorText>{error}</ErrorText>
            <Button disabled={!picked} loading={vote.isPending} onPress={() => pick != null && vote.mutate(pick)}>
              {picked ? (
                <Text numberOfLines={1} style={{ flexShrink: 1, paddingHorizontal: 20, fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>
                  {tr('party.vote.voteFor', { name: picked.name })}
                </Text>
              ) : (
                tr('party.vote.pick')
              )}
            </Button>
          </>
        )
      }>
      <Header party={party} />
      <Question round={round} size={32} />
      <Countdown ms={ms} total={party.timer} />
      {late || voted ? (
        <Animated.View entering={motion.enter} style={{ gap: 16 }}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            {late ? <Hourglass size={20} color={t.accInk} /> : <Avatar member={mine} size={36} />}
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
              {late ? tr('party.vote.late') : mine ? tr('party.vote.myVote', { name: mine.name }) : tr('party.vote.done')}
            </Text>
            {!late && <Check size={20} color={t.accInk} />}
          </Card>
          <VotedLine round={round} byId={byId} />
        </Animated.View>
      ) : (
        <>
          {round.voted.length > 0 && <VotedLine round={round} byId={byId} />}
          {choices.length > BIG_GROUP ? (
            <MemberGrid>
              {choices.map((m) => (
                <MemberTile
                  key={m.id}
                  member={m}
                  cols={gridColumns(choices.length)}
                  role="radio"
                  selected={m.id === pick}
                  onPress={() => setPick(m.id)}
                />
              ))}
            </MemberGrid>
          ) : (
            <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
              {choices.map((m) => {
                const on = m.id === pick;
                return (
                  <PressableScale
                    key={m.id}
                    scale={0.97}
                    accessibilityRole="radio"
                    accessibilityState={{ selected: on }}
                    accessibilityLabel={m.name}
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
                    <Avatar member={m} />
                    <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
                      {m.name}
                    </Text>
                  </PressableScale>
                );
              })}
            </View>
          )}
        </>
      )}
    </Screen>
  );
}

/** The result: the winner big, the bars with who voted, reactions, and "Suivante". */
function Reveal({ party, round }: { party: Party; round: PartyRound }) {
  const t = useTheme();
  const qc = useQueryClient();
  const setParty = useSetParty(party.id);
  const [error, setError] = useState<string | null>(null);
  const byId = usePeople(party);
  const owner = isOwner(party);
  const refetch = () => qc.invalidateQueries({ queryKey: ['party', party.id] });
  const next = useMutation({
    mutationFn: (undo: boolean) => (undo ? partyApi.unnext(party.id) : partyApi.next(party.id)),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => {
      setError(partyError(e));
      refetch();
    },
  });
  const react = useMutation({
    mutationFn: (emoji: string) => partyApi.react(party.id, round.id, emoji),
    onSuccess: setParty,
    onError: refetch,
  });
  const list = tallies(round);
  return (
    <Screen
      tabs={false}
      footer={
        party.me && (
          <>
            <ErrorText>{error}</ErrorText>
            <NextButton round={round} owner={owner} busy={next.isPending} onPress={() => next.mutate(!owner && !!round.next?.mine)} />
          </>
        )
      }>
      <Header party={party} />
      <View style={{ gap: 8 }}>
        <Tag label={`#${round.number}`} tone="acc" />
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, lineHeight: 24, color: t.txt }}>{tidy(round.question)}</Text>
      </View>
      <View style={{ paddingVertical: 8 }}>
        <WinnerReveal round={round} byId={byId} size={112} />
      </View>
      {list.length > 0 && <ResultBars result={list} byId={byId} />}
      <Reactions round={round} disabled={!party.me} onReact={(e) => react.mutate(e)} />
    </Screen>
  );
}

const HISTORY_PAGE = 30;

/** Every question of the party with its winners, read from the room's history (all pages). */
function useMemory(id: number) {
  return useQuery({
    queryKey: ['party', id, 'memory'],
    queryFn: async () => {
      const all: HistoryItem[] = [];
      for (let before: number | null = null, i = 0; i < 20; i++) {
        const page: HistoryItem[] = await api<HistoryItem[]>(before == null ? '/history' : `/history?before=${before}`, { room: id });
        all.push(...page);
        if (page.length < HISTORY_PAGE) break;
        before = page[page.length - 1].number;
      }
      return all;
    },
  });
}

/** The party is over: what was asked and who won, then keep playing as a room (owner) or keep the account (guest). */
function Memory({ party }: { party: Party }) {
  const t = useTheme();
  const tr = useT();
  const memory = useMemory(party.id);
  const switchRoom = useSwitchRoom();
  const [error, setError] = useState<string | null>(null);
  const byId = usePeople(party);
  const owner = isOwner(party);
  const keep = useMutation({
    mutationFn: () => partyApi.keep(party.id),
    onMutate: () => setError(null),
    onSuccess: (r) => switchRoom(r?.id ?? party.id),
    onError: (e) => setError(partyError(e)),
  });
  const askKeep = async () => {
    if (await askConfirm(tr('party.memory.keepAsk'), tr('party.memory.keepMessage'), tr('common.continue'))) keep.mutate();
  };
  const guest = !!party.me?.guest;
  return (
    <Screen
      tabs={false}
      footer={
        (owner || guest) && (
          <>
            <ErrorText>{error}</ErrorText>
            {owner && (
              <Button loading={keep.isPending} onPress={askKeep}>
                {tr('party.memory.keep')}
              </Button>
            )}
            {guest &&
              (owner ? (
                <SoftButton onPress={() => router.push('/security')}>{tr('party.memory.signUp')}</SoftButton>
              ) : (
                <Button onPress={() => router.push('/security')}>{tr('party.memory.signUp')}</Button>
              ))}
          </>
        )
      }>
      <Header party={party} />
      <View style={{ gap: 6 }}>
        <Display size={34}>{tr('party.over')}</Display>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('party.memory.played', { count: party.played })}</Text>
      </View>
      {memory.isLoading && <ActivityIndicator color={t.mut} style={{ marginTop: 16 }} />}
      {memory.data?.map((h, i) => {
        const winners = h.winner_ids.map((wid) => byId.get(wid)).filter((m): m is Member => !!m);
        return (
          <Animated.View key={h.id} entering={motion.stagger(i)}>
            <Pressable accessibilityRole="button" onPress={() => router.push(`/round/${h.id}`)}>
              <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14 }}>
                <View style={{ flex: 1, gap: 8 }}>
                  <Text numberOfLines={3} style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}>
                    {tidy(h.question)}
                  </Text>
                  {winners.length > 0 && (
                    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                      <Avatar member={winners[0]} size={24} />
                      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 14, color: t.accInk }}>
                        {winners.map((m) => m.name).join(' · ')}
                      </Text>
                    </View>
                  )}
                </View>
                <ChevronRight size={18} color={t.mut} />
              </Card>
            </Pressable>
          </Animated.View>
        );
      })}
    </Screen>
  );
}
