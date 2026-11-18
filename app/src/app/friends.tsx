import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Check, ChevronLeft, Clock, Share2, UserPlus, X } from 'lucide-react-native';
import { type ReactNode, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';
import { QueryState } from '@/components/ui/query-state';
import { ScanButton } from '@/components/ui/qr-scanner';
import { Screen } from '@/components/ui/screen';
import { api, ApiError } from '@/lib/api/client';
import { t as tr, type TKey, useT } from '@/lib/i18n';
import { friendLink, friendMessage, shareText } from '@/lib/device/share';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { CodeText, Display } from '@/components/ui/text';
import { presetFriendCode } from '@/lib/device/launch-params';
import type { Member } from '@/lib/api/group';
import { type Person, useFriends, friendKeys } from '@/lib/api/friends';

const ERRORS: Record<string, TKey> = { bad_code: 'friends.errors.badCode', self_friend: 'friends.errors.self' };

const roomsText = (n: number) => tr('friends.roomsInCommon', { count: n });

/** A section title above a group of cards. */
function Label({ children }: { children: string }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: 4 }}>{children}</Text>;
}

/** One person: avatar, name, a grey line, and the actions on the right. */
function PersonRow({ person, sub, children }: { person: Person; sub?: string; children?: ReactNode }) {
  const t = useTheme();
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
      <Avatar member={{ id: person.user_id, name: person.name, color: person.color, photo_url: person.photo_url, avatar: person.avatar } as Member} size={40} />
      <View style={{ flex: 1, gap: 2 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {person.name}
        </Text>
        {!!sub && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
            {sub}
          </Text>
        )}
      </View>
      {children}
    </Card>
  );
}

/** A round icon button; `tone="acc"` for the one action that matters (accept). */
function Round({ label, onPress, disabled, tone, children }: { label: string; onPress: () => void; disabled?: boolean; tone?: 'acc'; children: ReactNode }) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      disabled={disabled}
      onPress={onPress}
      hitSlop={6}
      style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: tone === 'acc' ? t.acc : t.card2 }}>
      {children}
    </PressableScale>
  );
}

/** Requests to answer, my friend code (and QR), add someone by code, my friends, then people from my rooms. */
export default function FriendsScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const friendsQuery = useFriends();
  const data = friendsQuery.data;
  const [code, setCode] = useState(presetFriendCode() ?? '');
  const [note, setNote] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: friendKeys.all });
  // A code sends a request (or accepts theirs); an older server makes friends at once (no `status`).
  const add = useMutation({
    meta: { quiet: true },
    mutationFn: (value: string) => api<{ name: string; status?: string }>('/friends', { method: 'POST', json: { code: value.trim() } }),
    onSuccess: (r) => {
      setCode('');
      setNote(tr(r.status === 'requested' ? 'friends.requestSent' : 'friends.added', { name: r.name }));
      refresh();
    },
    onError: (e) => setNote(e instanceof ApiError && ERRORS[e.code] ? tr(ERRORS[e.code]) : tr('common.failed')),
  });
  // Ask a room-mate to stay friends for good, or accept someone's request.
  const keep = useMutation({
    mutationFn: (id: number) => api(`/friends/${id}`, { method: 'POST' }),
    onSettled: refresh,
  });
  // Remove a friend, decline a request or cancel mine.
  const remove = useMutation({
    mutationFn: (id: number) => api(`/friends/${id}`, { method: 'DELETE' }),
    onSettled: refresh,
  });
  const busy = keep.isPending || remove.isPending;
  const share = async () => {
    if (data && (await shareText(friendMessage(data.code))) === 'copied') {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    }
  };
  const friends = data?.friends.filter((f) => f.added) ?? [];
  const roomMates = data?.friends.filter((f) => !f.added) ?? [];
  const shown = new Set(data?.friends.map((f) => f.user_id));
  const incoming = data?.incoming ?? [];
  const outgoing = data?.outgoing?.filter((p) => !shown.has(p.user_id)) ?? [];

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('common.back')}
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        hitSlop={4}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>{tr('friends.title')}</Display>
      <QueryState query={friendsQuery} />

      {!!incoming.length && <Label>{tr('friends.requests')}</Label>}
      {incoming.map((p) => (
        <PersonRow key={p.user_id} person={p} sub={tr('friends.wantsToBeFriends')}>
          <Round label={tr('friends.declineA11y', { name: p.name })} disabled={busy} onPress={() => remove.mutate(p.user_id)}>
            <X size={16} color={t.mut} />
          </Round>
          <Round label={tr('friends.acceptA11y', { name: p.name })} disabled={busy} onPress={() => keep.mutate(p.user_id)} tone="acc">
            <Check size={18} color={t.accTxt} />
          </Round>
        </PersonRow>
      ))}

      {data && (
        <Card style={{ padding: 20, gap: 14, alignItems: 'center' }}>
          <View style={{ padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
            <QRCode value={friendLink(data.code)} size={168} color="#121216" backgroundColor="#fff" />
          </View>
          <View style={{ alignSelf: 'stretch', alignItems: 'center', gap: 2 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('friends.myCode')}</Text>
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
            <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>{copied ? tr('friends.linkCopied') : tr('friends.shareCode')}</Text>
          </Pressable>
        </Card>
      )}

      <Label>{tr('friends.addWithCode')}</Label>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel={tr('friends.code')}
          placeholder={tr('friends.code')}
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
            if (!s.friend) return setNote(tr('friends.notFriendQr'));
            setCode(s.friend);
            add.mutate(s.friend);
          }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('friends.addA11y')}
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

      {!!friends.length && <Label>{tr('friends.count', { count: friends.length })}</Label>}
      {friends.map((f) => (
        <PersonRow key={f.user_id} person={f} sub={f.rooms_in_common ? roomsText(f.rooms_in_common) : tr('friends.addedAsFriend')}>
          {/* Sharing a room keeps us friends anyway: only a friend from outside my rooms can be removed. */}
          {!f.rooms_in_common && (
            <Round label={tr('friends.removeA11y', { name: f.name })} disabled={busy} onPress={() => remove.mutate(f.user_id)}>
              <X size={16} color={t.mut} />
            </Round>
          )}
        </PersonRow>
      ))}

      {!!roomMates.length && <Label>{tr('friends.inRooms')}</Label>}
      {roomMates.map((f) =>
        f.requested ? (
          <PersonRow key={f.user_id} person={f} sub={tr('friends.pending')}>
            <Round label={tr('friends.cancelA11y', { name: f.name })} disabled={busy} onPress={() => remove.mutate(f.user_id)}>
              <Clock size={16} color={t.mut} />
            </Round>
          </PersonRow>
        ) : (
          <PersonRow key={f.user_id} person={f} sub={roomsText(f.rooms_in_common)}>
            <Round label={tr('friends.addNameA11y', { name: f.name })} disabled={busy} onPress={() => keep.mutate(f.user_id)}>
              <UserPlus size={16} color={t.txt} />
            </Round>
          </PersonRow>
        ),
      )}

      {!!outgoing.length && <Label>{tr('friends.sent')}</Label>}
      {outgoing.map((p) => (
        <PersonRow key={p.user_id} person={p} sub={tr('friends.pending')}>
          <Round label={tr('friends.cancelA11y', { name: p.name })} disabled={busy} onPress={() => remove.mutate(p.user_id)}>
            <X size={16} color={t.mut} />
          </Round>
        </PersonRow>
      ))}
    </Screen>
  );
}
