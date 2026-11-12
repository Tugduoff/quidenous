import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { ChevronLeft, Share2, UserPlus, X } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';

import { ScanButton } from '@/components/qr-scanner';
import { Screen } from '@/components/screen';
import { Avatar, Card, CodeText, Display } from '@/components/ui';
import { api, ApiError, presetFriendCode, type Member } from '@/lib/api';
import { keys, useFriends } from '@/lib/queries';
import { friendLink, friendMessage, shareText } from '@/lib/share';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, string> = { bad_code: 'Code ami inconnu', self_friend: "C'est ton propre code" };

/** My friend code (and QR), add someone by code, my friends. */
export default function FriendsScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data } = useFriends();
  const [code, setCode] = useState(presetFriendCode() ?? '');
  const [note, setNote] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const add = useMutation({
    mutationFn: (value: string) => api<{ name: string }>('/friends', { method: 'POST', json: { code: value.trim() } }),
    onSuccess: (r) => {
      setCode('');
      setNote(`${r.name} est ton ami`);
      qc.invalidateQueries({ queryKey: keys.friends });
    },
    onError: (e) => setNote(e instanceof ApiError ? (ERRORS[e.code] ?? 'Raté, réessaie') : 'Raté, réessaie'),
  });
  const remove = useMutation({
    mutationFn: (id: number) => api(`/friends/${id}`, { method: 'DELETE' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.friends }),
  });
  const share = async () => {
    if (data && (await shareText(friendMessage(data.code))) === 'copied') {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    }
  };

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>Amis</Display>

      {data && (
        <Card style={{ padding: 20, gap: 14, alignItems: 'center' }}>
          <View style={{ padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
            <QRCode value={friendLink(data.code)} size={168} color="#121216" backgroundColor="#fff" />
          </View>
          <View style={{ alignSelf: 'stretch', alignItems: 'center', gap: 2 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>Mon code ami</Text>
            <CodeText code={data.code} size={32} center />
          </View>
          <Pressable
            accessibilityRole="button"
            onPress={share}
            style={{
              alignSelf: 'stretch',
              height: 44,
              borderRadius: t.radius.pill,
              flexDirection: 'row',
              gap: 8,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.acc,
            }}>
            <Share2 size={18} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>{copied ? 'Lien copié' : 'Partager mon code'}</Text>
          </Pressable>
        </Card>
      )}

      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>Ajouter avec un code</Text>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel="Code ami"
          placeholder="Code ami"
          placeholderTextColor={t.mut}
          value={code}
          onChangeText={(v) => setCode(v.toUpperCase())}
          autoCapitalize="characters"
          maxLength={16}
          returnKeyType="go"
          onSubmitEditing={() => code.trim() && !add.isPending && add.mutate(code)}
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.bold, fontSize: 16, letterSpacing: 2, color: t.txt }}
        />
        <ScanButton
          onScan={(s) => {
            if (!s.friend) return setNote("Ce QR code n'est pas un code ami");
            setCode(s.friend);
            add.mutate(s.friend);
          }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Ajouter en ami"
          disabled={!code.trim() || add.isPending}
          onPress={() => add.mutate(code)}
          style={{
            width: 44,
            height: 44,
            borderRadius: 22,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: code.trim() ? t.acc : t.card2,
          }}>
          <UserPlus size={20} color={code.trim() ? t.accTxt : t.mut} />
        </Pressable>
      </Card>
      {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk, marginLeft: 4 }}>{note}</Text>}

      {!!data?.friends.length && (
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
          {`${data.friends.length} ami${data.friends.length > 1 ? 's' : ''}`}
        </Text>
      )}
      {data?.friends.map((f) => (
        <Card key={f.user_id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={{ id: f.user_id, name: f.name, color: f.color, photo_url: f.photo_url } as Member} size={40} />
          <View style={{ flex: 1, gap: 2 }}>
            <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
              {f.name}
            </Text>
            <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
              {f.rooms_in_common ? `${f.rooms_in_common} salon${f.rooms_in_common > 1 ? 's' : ''} en commun` : 'Ajouté avec son code'}
            </Text>
          </View>
          {f.added && !f.rooms_in_common && (
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={`Retirer ${f.name} de mes amis`}
              onPress={() => remove.mutate(f.user_id)}
              hitSlop={8}
              style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
              <X size={16} color={t.mut} />
            </Pressable>
          )}
        </Card>
      ))}
    </Screen>
  );
}
