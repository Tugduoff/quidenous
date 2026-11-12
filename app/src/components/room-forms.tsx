import { useMutation } from '@tanstack/react-query';
import { LogIn, Plus } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput } from 'react-native';

import { ScanButton } from '@/components/qr-scanner';
import { Card } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { authError } from '@/lib/auth';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, string> = {
  bad_code: 'Code inconnu',
  removed: 'Tu as été retiré de ce salon',
  bad_room_name: 'Donne-lui un nom (40 caractères max)',
};

/** Join a room with a code (or its QR code), or create one. `joinFirst` puts the code field on top. */
export function RoomForms({ initialCode, onEntered, joinFirst }: { initialCode?: string; onEntered: (id: number) => void; joinFirst?: boolean }) {
  const t = useTheme();
  const [name, setName] = useState('');
  const [code, setCode] = useState(initialCode ?? '');
  const [error, setError] = useState<string | null>(null);
  const fail = (e: unknown) => setError(e instanceof ApiError && ERRORS[e.code] ? ERRORS[e.code] : authError(e));

  const create = useMutation({
    mutationFn: () => api<{ id: number }>('/rooms', { method: 'POST', json: { name: name.trim() } }),
    onSuccess: (r) => onEntered(r.id),
    onError: fail,
  });
  const join = useMutation({
    mutationFn: (value: string) => api<{ id: number }>('/rooms/join', { method: 'POST', json: { code: value.trim() } }),
    onSuccess: (r) => onEntered(r.id),
    onError: fail,
  });
  const label = { fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 };
  const round = (on: boolean) => ({
    width: 44,
    height: 44,
    borderRadius: 22,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
    backgroundColor: on ? t.acc : t.card2,
  });

  const createForm = (
    <>
      <Text style={[label, { marginTop: 8 }]}>Nouveau salon</Text>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel="Nom du salon"
          placeholder="Les cousins, Le boulot…"
          placeholderTextColor={t.mut}
          value={name}
          onChangeText={setName}
          maxLength={40}
          returnKeyType="go"
          onSubmitEditing={() => name.trim() && !create.isPending && create.mutate()}
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Créer le salon"
          disabled={!name.trim() || create.isPending}
          onPress={() => create.mutate()}
          style={round(!!name.trim())}>
          <Plus size={20} color={name.trim() ? t.accTxt : t.mut} />
        </Pressable>
      </Card>
    </>
  );
  const joinForm = (
    <>
      <Text style={[label, joinFirst ? undefined : { marginTop: 8 }]}>Rejoindre avec un code</Text>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel="Code d'invitation"
          placeholder="ABC234"
          placeholderTextColor={t.mut}
          value={code}
          onChangeText={(v) => setCode(v.toUpperCase())}
          autoCapitalize="characters"
          autoCorrect={false}
          maxLength={20}
          returnKeyType="go"
          onSubmitEditing={() => code.trim() && !join.isPending && join.mutate(code)}
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.bold, fontSize: 16, letterSpacing: 2, color: t.txt }}
        />
        <ScanButton
          onScan={(s) => {
            if (!s.code) return setError("Ce QR code n'est pas celui d'un salon");
            setCode(s.code);
            join.mutate(s.code);
          }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Rejoindre"
          disabled={!code.trim() || join.isPending}
          onPress={() => join.mutate(code)}
          style={round(!!code.trim())}>
          <LogIn size={20} color={code.trim() ? t.accTxt : t.mut} />
        </Pressable>
      </Card>
    </>
  );

  return (
    <>
      {joinFirst ? joinForm : createForm}
      {joinFirst ? createForm : joinForm}
      {error && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, marginLeft: 4 }}>{error}</Text>}
    </>
  );
}
