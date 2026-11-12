import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Check, ChevronRight, Copy, RefreshCw, Share2, UserPlus } from 'lucide-react-native';
import { useState } from 'react';
import { Platform, Pressable, Text, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';

import { copyText } from '@/components/auth-ui';
import { askConfirm, Avatar, Card, CodeText, PressableScale } from '@/components/ui';
import { api } from '@/lib/api';
import { keys, useFriends, useGroup } from '@/lib/queries';
import { inviteLink, inviteMessage, shareText } from '@/lib/share';
import { useTheme } from '@/lib/theme';

/** `true` for a moment after a successful copy (a "Copié" tick). */
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

/** The room's invite: QR code, code, share the link, copy the code, and (owner, `rotate`) a new code. */
export function InviteCard({ qrSize = 168, rotate = true }: { qrSize?: number; rotate?: boolean }) {
  const t = useTheme();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const [shared, flashShared] = useFlash();
  const [copied, flashCopied] = useFlash();
  const renew = useMutation({
    mutationFn: () => api(`/rooms/${group!.id}/code`, { method: 'POST' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  if (!group) return null;
  const share = async () => flashShared((await shareText(inviteMessage(group.name, group.code))) === 'copied');
  // On a phone this opens the share sheet with just the code (it has "Copy"): no clipboard module needed.
  const copy = async () => flashCopied((await copyText(group.code)) && Platform.OS === 'web');
  const round = { width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 } as const;
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <View style={{ alignSelf: 'center', padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
        <QRCode value={inviteLink(group.code)} size={qrSize} color="#121216" backgroundColor="#fff" />
      </View>
      <View style={{ gap: 2 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>Code d'invitation</Text>
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
            {shared ? 'Lien copié' : 'Partager'}
          </Text>
        </PressableScale>
        <PressableScale accessibilityRole="button" accessibilityLabel={copied ? 'Code copié' : 'Copier le code'} onPress={copy} style={round}>
          {copied ? <Check size={18} color={t.accInk} /> : <Copy size={18} color={t.txt} />}
        </PressableScale>
        {rotate && group.my_role === 'owner' && (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel="Nouveau code"
            onPress={async () => {
              if (await askConfirm('Nouveau code ?', "L'ancien code et les liens déjà envoyés ne marcheront plus.", 'Changer')) renew.mutate();
            }}
            style={round}>
            <RefreshCw size={18} color={t.txt} />
          </PressableScale>
        )}
      </View>
    </Card>
  );
}

/**
 * Friends who aren't in this room yet, one tap to add them. With no friends at all, a way to add some
 * (unless `onlyIfAny`: the invite screen keeps to the invite); nothing when they are all here already.
 */
export function AddFriends({ onlyIfAny }: { onlyIfAny?: boolean }) {
  const t = useTheme();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const { data: friends } = useFriends();
  const add = useMutation({
    mutationFn: (userId: number) => api(`/rooms/${group!.id}/add`, { method: 'POST', json: { user_id: userId } }),
    onSettled: () => {
      qc.invalidateQueries({ queryKey: keys.group });
      qc.invalidateQueries({ queryKey: keys.friends });
      // A second person in the room opens the first question.
      qc.invalidateQueries({ queryKey: keys.current });
    },
  });
  if (!group || !friends) return null;
  const inRoom = new Set(group.members.map((m) => m.user_id));
  const outside = friends.friends.filter((f) => !inRoom.has(f.user_id));
  const none = friends.friends.length === 0;
  if (!outside.length && (onlyIfAny || !none)) return null;
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginHorizontal: 4 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>Ajouter des amis</Text>
        {!none && (
          <Pressable accessibilityRole="button" onPress={() => router.push('/friends')} hitSlop={8}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.accInk }}>Mes amis</Text>
          </Pressable>
        )}
      </View>
      {none && (
        <PressableScale scale={0.98} accessibilityRole="button" onPress={() => router.push('/friends')}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14 }}>
            <UserPlus size={18} color={t.accInk} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Pas encore d'amis</Text>
            <ChevronRight size={18} color={t.mut} />
          </Card>
        </PressableScale>
      )}
      {outside.map((f) => (
        <Card key={f.user_id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={{ id: f.user_id, name: f.name, color: f.color, photo_url: f.photo_url }} size={36} />
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
            {f.name}
          </Text>
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={`Ajouter ${f.name} au salon`}
            disabled={add.isPending}
            onPress={() => add.mutate(f.user_id)}
            hitSlop={6}
            style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.acc }}>
            <UserPlus size={17} color={t.accTxt} />
          </PressableScale>
        </Card>
      ))}
    </View>
  );
}
