import { useMutation } from '@tanstack/react-query';
import { UserPlus } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput } from 'react-native';
import { ScanButton } from '@/components/ui/qr-scanner';
import { ApiError } from '@/lib/api/client';
import { type TKey, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { presetFriendCode } from '@/lib/device/launch-params';
import { addFriend } from '@/lib/api/friends';
import { FriendsLabel } from '@/components/friends/person-row';

const ERRORS: Record<string, TKey> = { bad_code: 'friends.errors.badCode', self_friend: 'friends.errors.self' };

export function AddFriendCard({ onAdded }: { onAdded: () => void }) {
  const t = useTheme();
  const tr = useT();
  const [code, setCode] = useState(presetFriendCode() ?? '');
  const [note, setNote] = useState<string | null>(null);
  const add = useMutation({
    meta: { quiet: true },
    mutationFn: (value: string) => addFriend(value.trim()),
    onSuccess: (r) => {
      setCode('');
      setNote(tr(r.status === 'requested' ? 'friends.requestSent' : 'friends.added', { name: r.name }));
      onAdded();
    },
    onError: (e) => setNote(e instanceof ApiError && ERRORS[e.code] ? tr(ERRORS[e.code]) : tr('common.failed')),
  });
  return (
    <>
    <FriendsLabel>{tr('friends.addWithCode')}</FriendsLabel>
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
    </>
  );
}
