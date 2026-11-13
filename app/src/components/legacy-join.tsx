import { ArrowRight } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { ScanButton } from '@/components/qr-scanner';
import { Screen } from '@/components/screen';
import { Button, Card, Logo } from '@/components/ui';
import { ApiError, SERVER_FIELD } from '@/lib/api';
import { authError, pendingJoinCode } from '@/lib/auth';
import { type TKey, useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** Codes that read differently here than in `authError` (a room code, not an emailed one). */
const ERRORS: Record<string, TKey> = {
  bad_code: 'auth.legacy.errors.bad_code',
  bad_name: 'auth.legacy.errors.bad_name',
  bad_room_name: 'auth.legacy.errors.bad_room_name',
  removed: 'auth.legacy.errors.removed',
};

/** The entry screen of a server without accounts (older server): a first name and a room code, or a new room. */
export function LegacyJoin() {
  const t = useTheme();
  const tr = useT();
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
      setError(e instanceof ApiError && ERRORS[e.code] ? tr(ERRORS[e.code]) : authError(e));
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
          <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{creating ? tr('auth.legacy.create') : tr('common.join')}</Text>
          <ArrowRight size={20} color={t.accTxt} />
        </Button>
      }>
      <View style={{ marginTop: 48, gap: 14, marginBottom: 32 }}>
        <Logo size={44} />
        <Text style={{ fontFamily: t.font.regular, fontSize: 16, lineHeight: 22, color: t.mut }}>{tr('auth.tagline')}</Text>
      </View>
      <Card>
        <TextInput
          accessibilityLabel={tr('auth.name')}
          placeholder={tr('auth.name')}
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
            accessibilityLabel={tr('auth.legacy.roomName')}
            placeholder={tr('auth.legacy.roomName')}
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
              accessibilityLabel={tr('auth.legacy.groupCode')}
              placeholder={tr('auth.legacy.codePlaceholder')}
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
            {creating ? tr('auth.legacy.haveCode') : tr('auth.legacy.noCode')}
          </Text>
        </Pressable>
      )}
      {SERVER_FIELD && (
        <Card>
          <TextInput
            accessibilityLabel={tr('auth.legacy.server')}
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
