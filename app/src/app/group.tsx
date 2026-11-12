import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { router } from 'expo-router';
import { Bell, BellOff, Check, ChevronLeft, ChevronRight, Crown, Ellipsis, LogOut, Trash2, UserX } from 'lucide-react-native';
import { Pressable, Switch, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { AddFriends, InviteCard } from '@/components/invite';
import { ModerationDialog, type ModTarget } from '@/components/moderation';
import { Screen } from '@/components/screen';
import { askConfirm, Avatar, Card, Dialog, DialogTitle, Display, PressableScale } from '@/components/ui';
import { VibeCards, vibeIcon } from '@/components/vibes';
import { api, type RoomSummary } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { usePop } from '@/lib/motion';
import { keys, useGroup, useRooms, useVibes } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** Ask before anything that can't be undone with one tap, in the app's own dialog. */
const confirm = (message: string, title = 'Exclure du groupe ?', action = 'Voter') => askConfirm(title, message, action);

/** The other members, each with a vote to remove them. A majority of the others removes someone (never the owner, and only from 3 people). */
function Members() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me } = useSession();
  const { data: group } = useGroup();
  // "…" on a member: report, block / unblock.
  const [target, setTarget] = useState<ModTarget | null>(null);
  const qc2 = qc;
  const owner = group?.my_role === 'owner';
  const forceRemove = useMutation({
    mutationFn: (id: number) => api(`/rooms/${group!.id}/members/${id}`, { method: 'DELETE' }),
    onSettled: () => qc2.invalidateQueries({ queryKey: keys.group }),
  });
  const transfer = useMutation({
    mutationFn: (id: number) => api(`/rooms/${group!.id}/owner`, { method: 'POST', json: { member_id: id } }),
    onSettled: () => qc2.invalidateQueries({ queryKey: keys.group }),
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
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
        {owner
          ? 'Tu es propriétaire : tu peux retirer quelqu’un ou passer la main'
          : voteOn
            ? `Exclure quelqu'un demande ${group.kick_needed} vote${group.kick_needed > 1 ? 's' : ''}`
            : 'Membres'}
      </Text>
      {others.map((m) => {
        const state = group.kicks.find((k) => k.target_id === m.id);
        const mine = !!state?.mine;
        const onPress = async () => {
          if (!mine && !(await confirm(`Voter pour exclure ${m.name} ? Avec ${group.kick_needed} votes, ${m.name} sort du groupe.`)))
            return;
          kick.mutate({ id: m.id, undo: mine });
        };
        return (
          <Card key={m.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
            <Avatar member={m} size={36} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{m.name}</Text>
            {m.role === 'owner' && <Crown size={16} color={t.accInk} />}
            {owner ? (
              <View style={{ flexDirection: 'row', gap: 6 }}>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel={`Rendre ${m.name} propriétaire`}
                  onPress={async () => {
                    if (await confirm(`${m.name} devient propriétaire du salon, et toi simple membre.`, 'Passer la main ?', 'Confirmer'))
                      transfer.mutate(m.id);
                  }}
                  hitSlop={6}
                  style={{
                    width: 34,
                    height: 34,
                    borderRadius: 17,
                    alignItems: 'center',
                    justifyContent: 'center',
                    backgroundColor: t.card2,
                  }}>
                  <Crown size={15} color={t.mut} />
                </Pressable>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel={`Retirer ${m.name} du salon`}
                  onPress={async () => {
                    if (await confirm(`${m.name} sort du salon tout de suite.`, 'Retirer du salon ?', 'Retirer')) forceRemove.mutate(m.id);
                  }}
                  hitSlop={6}
                  style={{
                    flexDirection: 'row',
                    alignItems: 'center',
                    gap: 6,
                    height: 34,
                    paddingHorizontal: 12,
                    borderRadius: t.radius.pill,
                    backgroundColor: '#FF4FA322',
                  }}>
                  <UserX size={15} color={t.tag} />
                  <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.tag }}>Retirer</Text>
                </Pressable>
              </View>
            ) : voteOn && m.role !== 'owner' ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel={mine ? `Retirer mon vote contre ${m.name}` : `Voter pour exclure ${m.name}`}
                disabled={kick.isPending}
                onPress={onPress}
                hitSlop={6}
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 6,
                  height: 34,
                  paddingHorizontal: 12,
                  borderRadius: t.radius.pill,
                  backgroundColor: mine ? '#FF4FA322' : t.card2,
                }}>
                <UserX size={15} color={mine ? t.tag : t.mut} />
                <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: mine ? t.tag : t.mut }}>
                  {state ? `${state.count}/${group.kick_needed}` : 'Exclure'}
                </Text>
              </Pressable>
            ) : null}
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={`Plus d'actions pour ${m.name}`}
              onPress={() => setTarget({ kind: 'member', id: m.id, person: { user_id: m.user_id, name: m.name, color: m.color, photo: m.photo_url } })}
              hitSlop={6}
              style={{ width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
              <Ellipsis size={16} color={t.mut} />
            </Pressable>
          </Card>
        );
      })}
      <ModerationDialog target={target} onClose={() => setTarget(null)} />
    </View>
  );
}

/** Owner only: rename and the room's rules. */
function Settings() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const [name, setName] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: (body: Record<string, unknown>) => api(`/rooms/${group!.id}`, { method: 'PATCH', json: body }),
    onSettled: () => {
      qc.invalidateQueries({ queryKey: keys.group });
      qc.invalidateQueries({ queryKey: keys.rooms });
    },
  });
  if (!group || group.my_role !== 'owner') return null;
  const row = (label: string, hint: string, value: boolean, onChange: (v: boolean) => void) => (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
      <View style={{ flex: 1, gap: 2 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{label}</Text>
        <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>{hint}</Text>
      </View>
      <Switch value={value} onValueChange={onChange} trackColor={{ true: t.accInk, false: t.card2 }} thumbColor="#fff" />
    </View>
  );
  const draft = name ?? group.name;
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>Réglages du salon</Text>
      <Card style={{ padding: 16, gap: 16 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <TextInput
            accessibilityLabel="Nom du salon"
            value={draft}
            onChangeText={setName}
            maxLength={40}
            returnKeyType="done"
            onSubmitEditing={() => draft.trim() && draft !== group.name && save.mutate({ name: draft.trim() }, { onSuccess: () => setName(null) })}
            style={{
              flex: 1,
              height: 44,
              paddingHorizontal: 14,
              borderRadius: 14,
              backgroundColor: t.card2,
              fontFamily: t.font.semibold,
              fontSize: 16,
              color: t.txt,
            }}
          />
          <Pressable
            accessibilityRole="button"
            disabled={!draft.trim() || draft === group.name}
            onPress={() => save.mutate({ name: draft.trim() }, { onSuccess: () => setName(null) })}
            style={{
              height: 44,
              paddingHorizontal: 16,
              borderRadius: 14,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: draft !== group.name && draft.trim() ? t.acc : t.card2,
            }}>
            <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: draft !== group.name && draft.trim() ? t.accTxt : t.mut }}>
              OK
            </Text>
          </Pressable>
        </View>
        <VibeRow value={group.settings.vibe ?? null} onChange={(v) => save.mutate({ vibe: v ?? '' })} />
        {row('Passer quelqu’un à la majorité', 'Ne plus attendre celui qui ne vote pas', group.settings.skip_vote, (v) =>
          save.mutate({ skip_vote: v }),
        )}
        {row('Exclure à la majorité', 'Les membres peuvent voter pour retirer quelqu’un', group.settings.kick_vote, (v) =>
          save.mutate({ kick_vote: v }),
        )}
        {row('Questions réservées au proprio', 'Toi seul ajoutes ou importes des questions', group.settings.questions_by === 'owner', (v) =>
          save.mutate({ questions_by: v ? 'owner' : 'everyone' }),
        )}
      </Card>
    </View>
  );
}

/** Owner: the tone of the room's built-in questions, picked in a dialog (a server that has vibes). */
function VibeRow({ value, onChange }: { value: string | null; onChange: (vibe: string | null) => void }) {
  const t = useTheme();
  const features = useFeatures();
  const { data: vibes } = useVibes(features.vibes);
  const [open, setOpen] = useState(false);
  if (!features.vibes || !vibes?.length) return null;
  const current = vibes.find((v) => v.id === value);
  const Icon = vibeIcon(current?.icon);
  const pick = (v: string | null) => {
    setOpen(false);
    if (v !== value) onChange(v);
  };
  return (
    <>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityLabel={`Ambiance : ${current?.name ?? 'aucune'}. Changer`}
        onPress={() => setOpen(true)}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        <View style={{ flex: 1, gap: 2 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Ambiance</Text>
          <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>Le ton des questions de base</Text>
        </View>
        {current && <Icon size={16} color={t.accInk} />}
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: current ? t.txt : t.mut }}>{current?.name ?? 'Aucune'}</Text>
        <ChevronRight size={18} color={t.mut} />
      </PressableScale>
      <Dialog visible={open} onClose={() => setOpen(false)}>
        <DialogTitle title="Ambiance" />
        <VibeCards vibes={vibes} value={value} onChange={pick} />
        <Pressable
          accessibilityRole="radio"
          accessibilityState={{ selected: !current }}
          onPress={() => pick(null)}
          style={{ height: 44, flexDirection: 'row', gap: 6, alignItems: 'center', justifyContent: 'center' }}>
          {!current && <Check size={16} color={t.accInk} />}
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: current ? t.mut : t.accInk }}>Aucune</Text>
        </Pressable>
      </Dialog>
    </>
  );
}

/** Leave (anyone) and delete (owner). */
function Exit() {
  const t = useTheme();
  const qc = useQueryClient();
  const { setRoom } = useSession();
  const { data: group } = useGroup();
  if (!group) return null;
  const owner = group.my_role === 'owner';
  const done = async () => {
    await setRoom(null);
    qc.clear();
    const rooms = await api<{ id: number }[]>('/rooms').catch(() => []);
    if (rooms.length) {
      await setRoom(rooms[0].id);
      router.replace('/');
    } else router.replace('/rooms');
  };
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
    await done();
  };
  const remove = async () => {
    if (!(await confirm('Le salon disparaît pour tout le monde.', `Supprimer « ${group.name} » ?`, 'Supprimer'))) return;
    await api(`/rooms/${group.id}`, { method: 'DELETE' });
    await done();
  };
  const button = (label: string, Icon: typeof LogOut, onPress: () => void) => (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      style={{
        height: 48,
        borderRadius: t.radius.pill,
        flexDirection: 'row',
        gap: 8,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: '#FF4FA318',
      }}>
      <Icon size={18} color={t.tag} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.tag }}>{label}</Text>
    </Pressable>
  );
  return (
    <View style={{ gap: 10, marginTop: 8 }}>
      {button('Quitter le salon', LogOut, leave)}
      {owner && button('Supprimer le salon', Trash2, remove)}
    </View>
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

/** The group's own page: its name and its members (my profile stays about me). */
export default function GroupScreen() {
  const t = useTheme();
  const { me } = useSession();
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
        {group && muted !== undefined && <MuteButton id={group.id} muted={muted} />}
      </View>
      <View style={{ gap: 4 }}>
        <Display size={30}>{group?.name ?? 'Groupe'}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {group ? `${group.members.length} membre${group.members.length > 1 ? 's' : ''}${muted ? ' · en sourdine' : ''}` : ''}
        </Text>
      </View>
      {me && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={me} size={36} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{me.name}</Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>toi</Text>
        </Card>
      )}
      <InviteCard />
      <Members />
      <AddFriends />
      <Settings />
      <Exit />
    </Screen>
  );
}
