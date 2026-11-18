import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { router } from 'expo-router';
import { Bell, BellOff, ChevronLeft, Crown, Ellipsis, LogOut, Settings, UserX } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AddFriends, InviteCard } from '@/components/rooms/invite';
import { type ModAction, ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { StreakBadge } from '@/components/streaks/streaks';
import { MemberTitle } from '@/components/badges/title-chip';
import { showToast } from '@/lib/toast';
import { authError, useFeatures } from '@/lib/api/auth';
import { useT } from '@/lib/i18n';
import { usePop } from '@/lib/motion';
import { useRoomGone } from '@/lib/hooks/use-switch-room';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { Display } from '@/components/ui/text';
import { askConfirm } from '@/lib/confirm';
import { type RoomSummary, useRooms, roomKeys, setRoomMuted } from '@/lib/api/rooms';
import { useGroup, groupKeys, removeMember, voteKick, leaveRoom } from '@/lib/api/group';

/** Ask before leaving, in the app's own dialog. */
const confirm = (message: string, title: string, action: string) => askConfirm(title, message, action);

/** The "…" of a member row (36 px): remove, vote to remove, report, block. */
function MoreButton({ label, onPress }: { label: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      onPress={onPress}
      hitSlop={4}
      style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
      <Ellipsis size={16} color={t.mut} />
    </PressableScale>
  );
}

/**
 * The members, each with a "…": the owner removes someone there; with the vote on, the others vote there to remove them
 * (a majority of the others, never the owner, and only from 3 people). A vote in progress shows as a quiet count on the row.
 */
function Members() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me } = useSession();
  const { data: group } = useGroup();
  const [target, setTarget] = useState<ModTarget | null>(null);
  const owner = group?.my_role === 'owner';
  const forceRemove = useMutation({
    mutationFn: (id: number) => removeMember(group!.id, id),
    onSettled: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });
  const kick = useMutation({
    mutationFn: ({ id, undo }: { id: number; undo: boolean }) => voteKick(id, !undo),
    onSettled: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });
  if (!group) return null;
  // With two people a majority is one person deciding alone: no vote then.
  const voteOn = group.settings.kick_vote && group.members.length >= 3;
  const others = group.members.filter((m) => m.id !== me?.id);
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{tr('room.group.members')}</Text>
      {me && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={me} size={36} />
          <View style={{ flex: 1, gap: 3 }}>
            <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
              {me.name}
            </Text>
            <MemberTitle id={me.id} />
          </View>
          <StreakBadge id={me.id} />
          {owner && <Crown size={16} color={t.accInk} />}
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('room.group.you')}</Text>
        </Card>
      )}
      {others.map((m) => {
        const state = voteOn ? group.kicks.find((k) => k.target_id === m.id) : undefined;
        const mine = !!state?.mine;
        const votes = state ? `${state.count}/${group.kick_needed}` : undefined;
        const actions: ModAction[] = owner
          ? [
              {
                icon: UserX,
                label: tr('room.group.removeA11y', { name: m.name }),
                danger: true,
                confirm: { title: tr('room.group.removeAsk'), message: tr('room.group.removeMessage', { name: m.name }), label: tr('room.group.remove') },
                run: () => forceRemove.mutateAsync(m.id),
              },
            ]
          : voteOn && m.role !== 'owner'
            ? [
                {
                  icon: UserX,
                  label: tr(mine ? 'room.group.unvoteA11y' : 'room.group.voteA11y', { name: m.name }),
                  hint: votes,
                  danger: !mine,
                  confirm: mine
                    ? undefined
                    : {
                        title: tr('room.group.kickAsk', { name: m.name }),
                        message: tr('room.group.kickMessage', { name: m.name, count: group.kick_needed }),
                        label: tr('room.group.vote'),
                      },
                  run: () => kick.mutateAsync({ id: m.id, undo: mine }),
                },
              ]
            : [];
        return (
          <Card key={m.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
            <Avatar member={m} size={36} />
            <View style={{ flex: 1, gap: 3 }}>
              <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                {m.name}
              </Text>
              <MemberTitle id={m.id} />
            </View>
            <StreakBadge id={m.id} />
            {m.role === 'owner' && <Crown size={16} color={t.accInk} />}
            {state && (
              <View
                accessible
                accessibilityLabel={tr('room.group.kickVotesA11y', { count: state.count, total: group.kick_needed })}
                style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
                <UserX size={13} color={mine ? t.tagInk : t.mut} />
                <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: mine ? t.tagInk : t.mut }}>{votes}</Text>
              </View>
            )}
            <MoreButton
              label={tr('room.group.moreA11y', { name: m.name })}
              onPress={() =>
                setTarget({
                  kind: 'member',
                  id: m.id,
                  person: { user_id: m.user_id, name: m.name, color: m.color, photo: m.photo_url, avatar: m.avatar },
                  actions,
                })
              }
            />
          </Card>
        );
      })}
      <ModerationDialog target={target} onClose={() => setTarget(null)} />
    </View>
  );
}

/** Leave the room (anyone; deleting it is in the owner's settings). */
function Leave() {
  const t = useTheme();
  const tr = useT();
  const roomGone = useRoomGone();
  const { data: group } = useGroup();
  if (!group) return null;
  const owner = group.my_role === 'owner';
  const leave = async () => {
    // Leaving isn't final: the invite code brings me back, with my history.
    const msg = owner && group.members.length <= 1 ? tr('room.group.leaveAlone') : owner ? tr('room.group.leaveOwner') : tr('room.group.leaveMember');
    if (!(await confirm(msg, tr('room.group.leaveAsk'), tr('common.leave')))) return;
    try {
      await leaveRoom(group.id);
    } catch (e) {
      return showToast(authError(e));
    }
    await roomGone();
  };
  return (
    <Pressable
      accessibilityRole="button"
      onPress={leave}
      style={{
        height: 48,
        marginTop: 8,
        borderRadius: t.radius.pill,
        flexDirection: 'row',
        gap: 8,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: '#FF4FA318',
      }}>
      <LogOut size={18} color={t.tagInk} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.tagInk }}>{tr('room.group.leave')}</Text>
    </Pressable>
  );
}

/** Whether this room's pushes are silenced for me (undefined: an older server, or not loaded yet). */
function useMuted(id: number | undefined) {
  const features = useFeatures();
  const { data: rooms } = useRooms();
  if (!features.notificationSettings || id == null) return undefined;
  return rooms?.find((r) => r.id === id)?.muted;
}

/** Bell in the header: mute this room's pushes for me (the inbox still gets everything). */
function MuteButton({ id, muted }: { id: number; muted: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const pop = usePop(muted);
  const toggle = useMutation({
    mutationFn: (mute: boolean) => setRoomMuted(id, mute),
    onMutate: async (mute) => {
      await qc.cancelQueries({ queryKey: roomKeys.all });
      const before = qc.getQueryData<RoomSummary[]>(roomKeys.all);
      qc.setQueryData<RoomSummary[]>(roomKeys.all, (rooms) => rooms?.map((r) => (r.id === id ? { ...r, muted: mute } : r)));
      return { before };
    },
    onError: (_e, _mute, ctx) => {
      if (ctx?.before) qc.setQueryData(roomKeys.all, ctx.before);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: roomKeys.all }),
  });
  const Icon = muted ? BellOff : Bell;
  return (
    <PressableScale
      accessibilityRole="switch"
      accessibilityLabel={tr('room.group.notifications')}
      accessibilityState={{ checked: !muted }}
      onPress={() => toggle.mutate(!muted)}
      hitSlop={6}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: muted ? t.card2 : t.card }}>
      <Animated.View style={pop}>
        <Icon size={20} color={muted ? t.mut : t.txt} />
      </Animated.View>
    </PressableScale>
  );
}

/** The group's own page: its name, invite and members (my profile stays about me); the owner's settings are behind the gear. */
export default function GroupScreen() {
  const t = useTheme();
  const tr = useT();
  const groupQuery = useGroup();
  const { data: group } = groupQuery;
  const muted = useMuted(group?.id);
  return (
    <Screen tabs={false}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('common.back')}
          onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
          hitSlop={4}
          style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
          <ChevronLeft size={22} color={t.txt} />
        </Pressable>
        <View style={{ flexDirection: 'row', gap: 8 }}>
          {group && muted !== undefined && <MuteButton id={group.id} muted={muted} />}
          {group?.my_role === 'owner' && (
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel={tr('room.settings.title')}
              onPress={() => router.push('/room-settings')}
              hitSlop={6}
              style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
              <Settings size={20} color={t.txt} />
            </PressableScale>
          )}
        </View>
      </View>
      <View style={{ gap: 4 }}>
        <Display size={30}>{group?.name ?? tr('room.group.fallbackName')}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {group ? tr('common.members', { count: group.members.length }) + (muted ? ` · ${tr('room.group.muted')}` : '') : ''}
        </Text>
      </View>
      <QueryState query={groupQuery} />
      <InviteCard rotate={false} />
      <Members />
      <AddFriends />
      <Leave />
    </Screen>
  );
}
