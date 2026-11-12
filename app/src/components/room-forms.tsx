import { useMutation } from '@tanstack/react-query';
import { ChevronLeft, LogIn, Plus } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { ScanButton } from '@/components/qr-scanner';
import { Button, Card, Display } from '@/components/ui';
import { VibeCards } from '@/components/vibes';
import { api, ApiError } from '@/lib/api';
import { authError, useFeatures } from '@/lib/auth';
import { motion } from '@/lib/motion';
import { useVibes } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, string> = {
  bad_code: 'Code inconnu',
  removed: 'Tu as été retiré de ce salon',
  bad_room_name: 'Donne-lui un nom (40 caractères max)',
  bad_vibe: 'Ambiance inconnue',
};

/**
 * Join a room with a code (or its QR code), or create one. `joinFirst` puts the code field on top.
 * Creating takes a second step to pick the room's vibe (a server that has them); `onVibeStep` tells the screen, so it can clear the way.
 */
export function RoomForms({
  initialCode,
  onEntered,
  joinFirst,
  onVibeStep,
}: {
  initialCode?: string;
  onEntered: (id: number) => void;
  joinFirst?: boolean;
  onVibeStep?: (on: boolean) => void;
}) {
  const t = useTheme();
  const [name, setName] = useState('');
  const [code, setCode] = useState(initialCode ?? '');
  const [error, setError] = useState<string | null>(null);
  const fail = (e: unknown) => setError(e instanceof ApiError && ERRORS[e.code] ? ERRORS[e.code] : authError(e));
  const features = useFeatures();
  const { data: vibes } = useVibes(features.vibes);
  const [picking, setPicking] = useState(false);
  const [vibe, setVibe] = useState<string | null>(null);
  const showVibes = (on: boolean) => {
    setPicking(on);
    setError(null);
    onVibeStep?.(on);
  };

  const create = useMutation({
    mutationFn: (withVibe: string | null) =>
      api<{ id: number }>('/rooms', { method: 'POST', json: withVibe ? { name: name.trim(), vibe: withVibe } : { name: name.trim() } }),
    onSuccess: (r) => onEntered(r.id),
    onError: fail,
  });
  // Named: on to the vibes when there are some to pick from, else straight in.
  const next = () => {
    if (!name.trim() || create.isPending) return;
    if (vibes?.length) showVibes(true);
    else create.mutate(null);
  };
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
          onSubmitEditing={next}
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Créer le salon"
          disabled={!name.trim() || create.isPending}
          onPress={next}
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

  const errorLine = error && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, marginLeft: 4 }}>{error}</Text>;

  if (picking && vibes)
    return (
      <Animated.View entering={motion.enter} style={{ gap: 16 }}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Retour au nom du salon"
          onPress={() => showVibes(false)}
          hitSlop={8}
          style={{ flexDirection: 'row', alignItems: 'center', gap: 4, alignSelf: 'flex-start' }}>
          <ChevronLeft size={18} color={t.mut} />
          <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut, maxWidth: 240 }}>
            {name.trim()}
          </Text>
        </Pressable>
        <Display size={28}>Quelle ambiance ?</Display>
        <VibeCards vibes={vibes} value={vibe} onChange={setVibe} />
        {errorLine}
        <View style={{ gap: 6 }}>
          <Button disabled={!vibe} loading={create.isPending && create.variables != null} onPress={() => vibe && create.mutate(vibe)}>
            Créer
          </Button>
          <Pressable
            accessibilityRole="button"
            disabled={create.isPending}
            onPress={() => create.mutate(null)}
            style={{ height: 44, alignItems: 'center', justifyContent: 'center' }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>Passer</Text>
          </Pressable>
        </View>
      </Animated.View>
    );

  return (
    <>
      {joinFirst ? joinForm : createForm}
      {joinFirst ? createForm : joinForm}
      {errorLine}
    </>
  );
}
