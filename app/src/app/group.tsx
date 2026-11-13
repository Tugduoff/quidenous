import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type ReactNode, useState } from 'react';
import { router } from 'expo-router';
import { Bell, BellOff, ChevronLeft, Crown, Ellipsis, LogOut, Settings, UserX } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { AddFriends, InviteCard } from '@/components/invite';
import { ModerationDialog, type ModTarget } from '@/components/moderation';
import { Screen } from '@/components/screen';
import { askConfirm, Avatar, Card, Display, PressableScale } from '@/components/ui';
import { api, type RoomSummary } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { usePop } from '@/lib/motion';
import { keys, useGroup, useRooms } from '@/lib/queries';
import { useRoomGone } from '@/lib/rooms';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** Ask before anything that can't be undone with one tap, in the app's own dialog. */
const confirm = (message: string, title: string, action: string) => askConfirm(title, message, action);

/** The round icon button of a member row (36 px, like the "…" next to it). */
function RowButton({
  label,
  onPress,
  disabled,
  tone = 'plain',
  badge,
  children,
}: {
  label: string;
  onPress: () => void;
  disabled?: boolean;
  tone?: 'plain' | 'danger';
  /** A small count on the corner (votes so far). */
  badge?: string;
  children: ReactNode;
}) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      disabled={disabled}
      onPress={onPress}
      hitSlop={4}
      style={{
        width: 36,
        height: 36,
        borderRadius: 18,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: tone === 'danger' ? '#FF4FA322' : t.card2,
      }}>
      {children}
      {badge && (
        <View
          style={{
            position: 'absolute',
            top: -4,
            right: -6,
            minWidth: 20,
            height: 18,
            paddingHorizontal: 4,
            borderRadius: 9,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: t.tag,
            borderWidth: 2,
            borderColor: t.card,
          }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 9, color: '#fff' }}>{badge}</Text>
        </View>
      )}
    </PressableScale>
  );
}

/** The other members, each with a vote to remove them. A majority of the others removes someone (never the owner, and only from 3 people). */
function Members() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me } = useSession();
  const { data: group } = useGroup();
  // "…" on a member: report, block / unblock.
  const [target, setTarget] = useState<ModTarget | null>(null);
  const owner = group?.my_role === 'owner';
  const forceRemove = useMutation({
    mutationFn: (id: number) => api(`/rooms/${group!.id}/members/${id}`, { method: 'DELETE' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  const kick = useMutation({
    mutationFn: ({ id, undo }: { id: number; undo: boolean }) => api(`/members/${id}/kick`, { method: undo ? 'DELETE' : 'PUT' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  if (!group) return null;
  // With two people a majority is one person deciding alone: no vote then.
  const voteOn = group.settings.kick_vote && group.members.length >= 3;
  const others = group.members.filter((m) => m.id !== me?.id);
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>Membres</Text>
      {me && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={me} size={36} />
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
            {me.name}
          </Text>
          {owner && <Crown size={16} color={t.accInk} />}
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>toi</Text>
        </Card>
      )}
      {others.map((m) => {
        const state = group.kicks.find((k) => k.target_id === m.id);
        const mine = !!state?.mine;
        const onPress = async () => {
          if (!mine && !(await confirm(`${m.name} sort du salon à ${group.kick_needed} votes.`, `Exclure ${m.name} ?`, 'Voter'))) return;
          kick.mutate({ id: m.id, undo: mine });
        };
        return (
          <Card key={m.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
            <Avatar member={m} size={36} />
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
              {m.name}
            </Text>
            {m.role === 'owner' && <Crown size={16} color={t.accInk} />}
            {owner ? (
              <RowButton
                label={`Retirer ${m.name} du salon`}
                tone="danger"
                onPress={async () => {
                  if (await confirm(`${m.name} sort du salon tout de suite.`, 'Retirer du salon ?', 'Retirer')) forceRemove.mutate(m.id);
                }}>
                <UserX size={16} color={t.tag} />
              </RowButton>
            ) : voteOn && m.role !== 'owner' ? (
              <RowButton
                label={
                  (mine ? `Retirer mon vote contre ${m.name}` : `Voter pour exclure ${m.name}`) +
                  (state ? `, ${state.count} vote${state.count > 1 ? 's' : ''} sur ${group.kick_needed}` : '')
                }
                tone={mine ? 'danger' : 'plain'}
                disabled={kick.isPending}
                onPress={onPress}
                badge={state ? `${state.count}/${group.kick_needed}` : undefined}>
                <UserX size={16} color={mine ? t.tag : t.mut} />
              </RowButton>
            ) : null}
            <RowButton
              label={`Plus d'actions pour ${m.name}`}
              onPress={() =>
                setTarget({ kind: 'member', id: m.id, person: { user_id: m.user_id, name: m.name, color: m.color, photo: m.photo_url } })
              }>
              <Ellipsis size={16} color={t.mut} />
            </RowButton>
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
  const roomGone = useRoomGone();
  const { data: group } = useGroup();
  if (!group) return null;
  const owner = group.my_role === 'owner';
  const leave = async () => {
    // Leaving isn't final: the invite code brings me back, with my history.
    const msg =
      owner && group.members.length <= 1
        ? 'Tu es seul : le salon sera supprimé.'
        : owner
          ? "Le membre le plus ancien devient propriétaire. Tu pourras revenir avec le code d'invitation."
          : "Tu pourras revenir avec le code d'invitation.";
    if (!(await confirm(msg, 'Quitter le salon ?', 'Quitter'))) return;
    await api(`/rooms/${group.id}/leave`, { method: 'POST' });
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
      <LogOut size={18} color={t.tag} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.tag }}>Quitter le salon</Text>
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
  const qc = useQueryClient();
  const pop = usePop(muted);
  const toggle = useMutation({
    mutationFn: (mute: boolean) => api<{ ok: boolean; muted: boolean }>(`/rooms/${id}/mute`, { method: mute ? 'PUT' : 'DELETE' }),
    onMutate: async (mute) => {
      await qc.cancelQueries({ queryKey: keys.rooms });
      const before = qc.getQueryData<RoomSummary[]>(keys.rooms);
      qc.setQueryData<RoomSummary[]>(keys.rooms, (rooms) => rooms?.map((r) => (r.id === id ? { ...r, muted: mute } : r)));
      return { before };
    },
    onError: (_e, _mute, ctx) => {
      if (ctx?.before) qc.setQueryData(keys.rooms, ctx.before);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: keys.rooms }),
  });
  const Icon = muted ? BellOff : Bell;
  return (
    <PressableScale
      accessibilityRole="switch"
      accessibilityLabel="Notifications du salon"
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
  const { data: group } = useGroup();
  const muted = useMuted(group?.id);
  return (
    <Screen tabs={false}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Retour"
          onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
          style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
          <ChevronLeft size={22} color={t.txt} />
        </Pressable>
        <View style={{ flexDirection: 'row', gap: 8 }}>
          {group && muted !== undefined && <MuteButton id={group.id} muted={muted} />}
          {group?.my_role === 'owner' && (
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel="Paramètres du salon"
              onPress={() => router.push('/room-settings')}
              hitSlop={6}
              style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
              <Settings size={20} color={t.txt} />
            </PressableScale>
          )}
        </View>
      </View>
      <View style={{ gap: 4 }}>
        <Display size={30}>{group?.name ?? 'Salon'}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {group ? `${group.members.length} membre${group.members.length > 1 ? 's' : ''}${muted ? ' · en sourdine' : ''}` : ''}
        </Text>
      </View>
      <InviteCard rotate={false} />
      <Members />
      <AddFriends />
      <Leave />
    </Screen>
  );
}
