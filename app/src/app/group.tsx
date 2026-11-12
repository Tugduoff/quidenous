import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { router } from 'expo-router';
import { ChevronLeft, Crown, LogOut, RefreshCw, Share2, Trash2, UserPlus, UserX } from 'lucide-react-native';
import { Alert, Platform, Pressable, Switch, Text, TextInput, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';

import { Screen } from '@/components/screen';
import { Avatar, Card, Display } from '@/components/ui';
import { api } from '@/lib/api';
import { keys, useFriends, useGroup } from '@/lib/queries';
import { inviteLink, inviteMessage, shareText } from '@/lib/share';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** Ask before voting someone out (a vote can be taken back). */
function confirm(message: string, title = 'Exclure du groupe ?', action = 'Voter'): Promise<boolean> {
  if (Platform.OS === 'web') return Promise.resolve(typeof window === 'undefined' || window.confirm(`${title}\n${message}`));
  return new Promise((resolve) =>
    Alert.alert(title, message, [
      { text: 'Annuler', style: 'cancel', onPress: () => resolve(false) },
      { text: action, style: 'destructive', onPress: () => resolve(true) },
    ]),
  );
}

/** The other members, each with a vote to remove them. A majority of the others removes someone. */
function Members() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me } = useSession();
  const { data: group } = useGroup();
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
  const voteOn = group.settings.kick_vote;
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
            ) : voteOn ? (
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
          </Card>
        );
      })}
    </View>
  );
}

/** The invite code, a share button, and (owner) a new code. */
function Invite() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const [copied, setCopied] = useState(false);
  const rotate = useMutation({
    mutationFn: () => api(`/rooms/${group!.id}/code`, { method: 'POST' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  if (!group) return null;
  const share = async () => {
    if ((await shareText(inviteMessage(group.name, group.code))) === 'copied') {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    }
  };
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <View style={{ alignSelf: 'center', padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
        <QRCode value={inviteLink(group.code)} size={168} color="#121216" backgroundColor="#fff" />
      </View>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>Code d'invitation</Text>
      <Text selectable style={{ fontFamily: t.font.display, fontSize: 34, letterSpacing: 4, color: t.txt }}>
        {group.code}
      </Text>
      <View style={{ flexDirection: 'row', gap: 10 }}>
        <Pressable
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
          <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>{copied ? 'Lien copié' : "Partager l'invitation"}</Text>
        </Pressable>
        {group.my_role === 'owner' && (
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Nouveau code"
            onPress={async () => {
              if (await confirm("L'ancien code et les liens déjà envoyés ne marcheront plus.", 'Nouveau code ?', 'Changer'))
                rotate.mutate();
            }}
            style={{ width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
            <RefreshCw size={18} color={t.txt} />
          </Pressable>
        )}
      </View>
    </Card>
  );
}

/** Friends who aren't in this room yet, one tap to add them. */
function AddFriends() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const { data: friends } = useFriends();
  const add = useMutation({
    mutationFn: (userId: number) => api(`/rooms/${group!.id}/add`, { method: 'POST', json: { user_id: userId } }),
    onSettled: () => {
      qc.invalidateQueries({ queryKey: keys.group });
      qc.invalidateQueries({ queryKey: keys.friends });
    },
  });
  if (!group || !friends) return null;
  const inRoom = new Set(group.members.map((m) => m.user_id));
  const outside = friends.friends.filter((f) => !inRoom.has(f.user_id));
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginHorizontal: 4 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>Ajouter des amis</Text>
        <Pressable accessibilityRole="button" onPress={() => router.push('/friends')} hitSlop={8}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.accInk }}>Mes amis</Text>
        </Pressable>
      </View>
      {outside.length === 0 && (
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut, marginHorizontal: 4 }}>
          Tous tes amis sont déjà là. Ajoutes-en avec leur code ami.
        </Text>
      )}
      {outside.map((f) => (
        <Card key={f.user_id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={{ id: f.user_id, name: f.name, color: f.color, photo_url: f.photo_url }} size={36} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{f.name}</Text>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={`Ajouter ${f.name} au salon`}
            disabled={add.isPending}
            onPress={() => add.mutate(f.user_id)}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 34,
              paddingHorizontal: 12,
              borderRadius: t.radius.pill,
              backgroundColor: t.acc,
            }}>
            <UserPlus size={15} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 13, color: t.accTxt }}>Ajouter</Text>
          </Pressable>
        </Card>
      ))}
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
    const msg = owner
      ? 'Le membre le plus ancien devient propriétaire. Seul dans le salon, il est supprimé.'
      : 'Tu ne verras plus ce salon. Il faudra un code pour revenir.';
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

/** The group's own page: its name and its members (my profile stays about me). */
export default function GroupScreen() {
  const t = useTheme();
  const { me } = useSession();
  const { data: group } = useGroup();
  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <View style={{ gap: 4 }}>
        <Display size={30}>{group?.name ?? 'Groupe'}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {group ? `${group.members.length} membre${group.members.length > 1 ? 's' : ''}` : ''}
        </Text>
      </View>
      {me && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={me} size={36} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{me.name}</Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>toi</Text>
        </Card>
      )}
      <Invite />
      <Members />
      <AddFriends />
      <Settings />
      <Exit />
    </Screen>
  );
}
