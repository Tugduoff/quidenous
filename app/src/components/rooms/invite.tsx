import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Check, ChevronRight, Copy, RefreshCw, Search, Share2, UserPlus, X } from 'lucide-react-native';
import { useState } from 'react';
import { Platform, Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';
import { useT } from '@/lib/i18n';
import { shareText } from '@/lib/device/share';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { CodeText } from '@/components/ui/text';
import { askConfirm } from '@/lib/confirm';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { copyText } from '@/lib/device/clipboard';
import { type Friend, useFriends } from '@/lib/api/friends';
import { useGroup, groupKeys, renewRoomCode } from '@/lib/api/group';
import { useInvites, useRoomInvites, roomKeys, addToRoom, cancelRoomInvite, acceptInvite, declineInvite } from '@/lib/api/rooms';
import { roundKeys } from '@/lib/api/rounds';
import { inviteLink, inviteMessage } from '@/lib/links';
import { useFeatures } from '@/lib/api/features';

function useFlash(): [boolean, (ok: boolean) => void] {
  const [on, setOn] = useState(false);
  return [
    on,
    (ok) => {
      if (!ok) return;
      setOn(true);
      setTimeout(() => setOn(false), 1800);
    },
  ];
}

export function InviteCard({ qrSize = 168, rotate = true }: { qrSize?: number; rotate?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const [shared, flashShared] = useFlash();
  const [copied, flashCopied] = useFlash();
  const renew = useMutation({
    mutationFn: () => (group ? renewRoomCode(group.id) : Promise.reject(new Error('no room'))),
    onSettled: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });
  if (!group) return null;
  const share = async () => flashShared((await shareText(inviteMessage(group.name, group.code))) === 'copied');
  const copy = async () => flashCopied((await copyText(group.code)) && Platform.OS === 'web');
  const round = { width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 } as const;
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <View style={{ alignSelf: 'center', padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
        <QRCode value={inviteLink(group.code)} size={qrSize} color="#121216" backgroundColor="#fff" />
      </View>
      <View style={{ gap: 2 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('room.invite.code')}</Text>
        <CodeText code={group.code} />
      </View>
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
            {shared ? tr('room.invite.linkCopied') : tr('common.share')}
          </Text>
        </PressableScale>
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={copied ? tr('room.invite.codeCopied') : tr('room.invite.copyCode')}
          onPress={copy}
          style={round}>
          {copied ? <Check size={18} color={t.accInk} /> : <Copy size={18} color={t.txt} />}
        </PressableScale>
        {rotate && group.my_role === 'owner' && (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('room.invite.newCode')}
            onPress={async () => {
              if (await askConfirm(tr('room.invite.newCodeAsk'), tr('room.invite.newCodeMessage'), tr('room.invite.change'))) renew.mutate();
            }}
            style={round}>
            <RefreshCw size={18} color={t.txt} />
          </PressableScale>
        )}
      </View>
    </Card>
  );
}

export function AddFriends({ onlyIfAny }: { onlyIfAny?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const { data: group } = useGroup();
  const { data: friends } = useFriends();
  const [open, setOpen] = useState(false);
  if (!group || !friends) return null;
  const inRoom = new Set(group.members.map((m) => m.user_id));
  const outside = friends.friends.filter((f) => !inRoom.has(f.user_id));
  const none = friends.friends.length === 0;
  if (!outside.length && (onlyIfAny || !none)) return null;
  const row = (title: string, onPress: () => void, sub?: string) => (
    <PressableScale scale={0.98} accessibilityRole="button" onPress={onPress}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14 }}>
        <View style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.accSoft }}>
          <UserPlus size={18} color={t.accInk} />
        </View>
        <View style={{ flex: 1, gap: 1 }}>
          <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
            {title}
          </Text>
          {!!sub && (
            <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
              {sub}
            </Text>
          )}
        </View>
        <ChevronRight size={18} color={t.mut} />
      </Card>
    </PressableScale>
  );
  if (none) return row(tr('room.invite.noFriends'), () => router.push('/friends'));
  return (
    <>
      {row(tr('room.invite.addFriends'), () => setOpen(true), tr('room.invite.outside', { count: outside.length }))}
      <InviteSheet visible={open} onClose={() => setOpen(false)} room={group.id} friends={outside} />
    </>
  );
}

function InviteSheet({ visible, onClose, room, friends }: { visible: boolean; onClose: () => void; room: number; friends: Friend[] }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [q, setQ] = useState('');
  const { data: pending } = useRoomInvites(room, visible);
  const invited = new Set(pending ?? []);
  const refresh = () => {
    qc.invalidateQueries({ queryKey: roomKeys.roomInvites(room) });
    qc.invalidateQueries({ queryKey: groupKeys.all });
    qc.invalidateQueries({ queryKey: roundKeys.current });
  };
  const invite = useMutation({
    mutationFn: (userId: number) => addToRoom(room, userId),
    onSettled: refresh,
  });
  const cancel = useMutation({
    mutationFn: (userId: number) => cancelRoomInvite(room, userId),
    onSettled: refresh,
  });
  const needle = q.trim().toLocaleLowerCase();
  const shown = needle ? friends.filter((f) => f.name.toLocaleLowerCase().includes(needle)) : friends;
  const close = () => {
    setQ('');
    onClose();
  };
  return (
    <Dialog visible={visible} onClose={close}>
      <DialogTitle title={tr('room.invite.addFriends')} />
      {friends.length > 5 && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, height: 44, paddingHorizontal: 14, borderRadius: 22, backgroundColor: t.card2 }}>
          <Search size={16} color={t.mut} />
          <TextInput
            accessibilityLabel={tr('room.invite.search')}
            placeholder={tr('room.invite.search')}
            placeholderTextColor={t.mut}
            value={q}
            onChangeText={setQ}
            autoCorrect={false}
            style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
          />
        </View>
      )}
      <ScrollView style={{ maxHeight: 340 }} contentContainerStyle={{ gap: 4 }} keyboardShouldPersistTaps="handled">
        {shown.map((f) => {
          const on = invited.has(f.user_id);
          return (
            <View key={f.user_id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, paddingVertical: 6 }}>
              <Avatar member={{ id: f.user_id, name: f.name, color: f.color, photo_url: f.photo_url, avatar: f.avatar }} size={36} />
              <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                {f.name}
              </Text>
              <PressableScale
                accessibilityRole="button"
                accessibilityLabel={on ? tr('room.invite.cancelA11y', { name: f.name }) : tr('room.invite.addToRoom', { name: f.name })}
                disabled={invite.isPending || cancel.isPending}
                onPress={() => (on ? cancel.mutate(f.user_id) : invite.mutate(f.user_id))}
                hitSlop={6}
                style={{
                  height: 34,
                  paddingHorizontal: 12,
                  borderRadius: 17,
                  flexDirection: 'row',
                  gap: 6,
                  alignItems: 'center',
                  justifyContent: 'center',
                  backgroundColor: on ? 'transparent' : t.card2,
                  borderWidth: on ? 1 : 0,
                  borderColor: t.line,
                }}>
                {on ? <Check size={15} color={t.accInk} /> : <UserPlus size={15} color={t.txt} />}
                <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: on ? t.mut : t.txt }}>
                  {on ? tr('room.invite.invited') : tr('room.invite.invite')}
                </Text>
              </PressableScale>
            </View>
          );
        })}
        {!shown.length && <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut, paddingVertical: 8 }}>{tr('room.invite.noMatch')}</Text>}
      </ScrollView>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginTop: 4 }}>
        <Pressable
          accessibilityRole="button"
          onPress={() => {
            close();
            router.push('/friends');
          }}
          hitSlop={8}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{tr('room.invite.myFriends')}</Text>
        </Pressable>
        <Pressable accessibilityRole="button" onPress={close} hitSlop={8}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{tr('common.close')}</Text>
        </Pressable>
      </View>
    </Dialog>
  );
}

export function RoomInvites({ onJoined }: { onJoined: (room: number) => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { consent } = useFeatures();
  const { data: invites } = useInvites(consent);
  const accept = useMutation({
    mutationFn: (id: number) => acceptInvite(id),
    onSuccess: (r) => onJoined(r.room_id),
    onSettled: () => qc.invalidateQueries({ queryKey: roomKeys.invites }),
  });
  const decline = useMutation({
    mutationFn: (id: number) => declineInvite(id),
    onSettled: () => qc.invalidateQueries({ queryKey: roomKeys.invites }),
  });
  if (!invites?.length) return null;
  const busy = accept.isPending || decline.isPending;
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{tr('room.invites.title')}</Text>
      {invites.map((i) => (
        <Card key={i.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12, paddingLeft: 14 }}>
          <Avatar member={{ id: i.from.user_id, name: i.from.name, color: i.from.color, photo_url: i.from.photo_url, avatar: i.from.avatar }} size={36} />
          <View style={{ flex: 1, gap: 1 }}>
            <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
              {i.room_name}
            </Text>
            <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
              {tr('room.invites.from', { name: i.from.name })}
            </Text>
          </View>
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('room.invites.declineA11y', { room: i.room_name })}
            disabled={busy}
            onPress={() => decline.mutate(i.id)}
            hitSlop={6}
            style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
            <X size={16} color={t.mut} />
          </PressableScale>
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('room.invites.acceptA11y', { room: i.room_name })}
            disabled={busy}
            onPress={() => accept.mutate(i.id)}
            style={{ height: 36, paddingHorizontal: 14, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.acc }}>
            <Text style={{ fontFamily: t.font.bold, fontSize: 13, color: t.accTxt }}>{tr('room.invites.accept')}</Text>
          </PressableScale>
        </Card>
      ))}
    </View>
  );
}
