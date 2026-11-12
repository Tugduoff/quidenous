import { ArrowRight } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { ScanButton } from '@/components/qr-scanner';
import { Screen } from '@/components/screen';
import { Button, Card, Logo } from '@/components/ui';
import { ApiError, SERVER_FIELD } from '@/lib/api';
import { authError, pendingJoinCode } from '@/lib/auth';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, string> = {
  bad_code: 'Code inconnu',
  bad_name: 'Prénom invalide',
  bad_room_name: 'Donne un nom à ton salon',
  removed: 'Tu as été retiré de ce salon',
};

/** The entry screen of a server without accounts (older server): a first name and a room code, or a new room. */
export function LegacyJoin() {
  const t = useTheme();
  const { join, start } = useSession();
  const [name, setName] = useState('');
  const [preset] = useState(pendingJoinCode);
  const [code, setCode] = useState(preset ?? '');
  // Without an invite: join with a code, or start a room of my own.
  const [creating, setCreating] = useState(false);
  const [room, setRoom] = useState('');
  const [server, setServer] = useState('https://');
  const [busy, setBusy] = useState(false);
  const next = useRef<ComponentRef<typeof TextInput>>(null);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      if (creating) await start(name, room, SERVER_FIELD ? server : undefined);
      else await join(code, name, SERVER_FIELD ? server : undefined);
    } catch (e) {
      setError(e instanceof ApiError && ERRORS[e.code] ? ERRORS[e.code] : authError(e));
      if (e instanceof ApiError && e.code === 'bad_code' && preset) setCode('');
    } finally {
      setBusy(false);
    }
  };

  const ready = !!name.trim() && !!(creating ? room.trim() : code.trim()) && !busy;
  const go = () => ready && submit();
  const input = { fontFamily: t.font.semibold, fontSize: 16, color: t.txt, padding: 16 };
  return (
    <Screen
      tabs={false}
      footer={
        <Button onPress={submit} loading={busy} disabled={!name.trim() || !(creating ? room.trim() : code.trim())}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{creating ? 'Créer mon salon' : 'Rejoindre'}</Text>
          <ArrowRight size={20} color={t.accTxt} />
        </Button>
      }>
      <View style={{ marginTop: 48, gap: 14, marginBottom: 32 }}>
        <Logo size={44} />
        <Text style={{ fontFamily: t.font.regular, fontSize: 16, lineHeight: 22, color: t.mut }}>
          Une question. Tout le monde vote.{'\n'}Personne n&apos;est à l&apos;abri.
        </Text>
      </View>
      <Card>
        <TextInput
          accessibilityLabel="Prénom"
          placeholder="Prénom"
          placeholderTextColor={t.mut}
          value={name}
          onChangeText={setName}
          autoComplete="given-name"
          returnKeyType="next"
          submitBehavior="submit"
          onSubmitEditing={() => (preset && code ? go() : next.current?.focus())}
          style={input}
        />
      </Card>
      {creating ? (
        <Card>
          <TextInput
            accessibilityLabel="Nom du salon"
            placeholder="Nom du salon"
            placeholderTextColor={t.mut}
            value={room}
            ref={next}
            onChangeText={setRoom}
            maxLength={40}
            returnKeyType="go"
            onSubmitEditing={go}
            style={input}
          />
        </Card>
      ) : (
        (!preset || !code) && (
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, paddingRight: 6 }}>
            <TextInput
              ref={next}
              accessibilityLabel="Code du groupe"
              placeholder="CODE"
              placeholderTextColor={t.mut}
              value={code}
              onChangeText={(v) => setCode(v.toUpperCase())}
              autoCapitalize="characters"
              autoCorrect={false}
              returnKeyType="go"
              onSubmitEditing={go}
              style={[input, { flex: 1, minWidth: 0, letterSpacing: 6 }]}
            />
            <ScanButton onScan={(s) => s.code && setCode(s.code)} />
          </Card>
        )
      )}
      {!preset && (
        <Pressable
          accessibilityRole="button"
          onPress={() => setCreating((c) => !c)}
          hitSlop={8}
          style={{ alignSelf: 'center', padding: 8 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.accInk }}>
            {creating ? "J'ai un code d'invitation" : 'Pas de code ? Crée ton salon'}
          </Text>
        </Pressable>
      )}
      {SERVER_FIELD && (
        <Card>
          <TextInput
            accessibilityLabel="Serveur"
            placeholder="https://…"
            placeholderTextColor={t.mut}
            value={server}
            onChangeText={setServer}
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="url"
            returnKeyType="go"
            onSubmitEditing={go}
            style={input}
          />
        </Card>
      )}
      {error && <Text style={{ fontFamily: t.font.semibold, color: t.tag }}>{error}</Text>}
    </Screen>
  );
}
