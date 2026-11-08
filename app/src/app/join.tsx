import { ArrowRight } from 'lucide-react-native';
import { useState } from 'react';
import { Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Button, Card, Logo } from '@/components/ui';
import { ApiError, presetJoinCode, SERVER_FIELD } from '@/lib/api';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, string> = {
  bad_code: 'Code inconnu',
  bad_name: 'Prénom invalide',
};

export default function Join() {
  const t = useTheme();
  const { join } = useSession();
  const [name, setName] = useState('');
  const [preset] = useState(presetJoinCode);
  const [code, setCode] = useState(preset ?? '');
  const [server, setServer] = useState('https://');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await join(code, name, SERVER_FIELD ? server : undefined);
    } catch (e) {
      setError(e instanceof ApiError ? (ERRORS[e.code] ?? 'Erreur') : 'Pas de connexion');
      if (e instanceof ApiError && e.code === 'bad_code' && preset) setCode('');
    } finally {
      setBusy(false);
    }
  };

  const input = { fontFamily: t.font.semibold, fontSize: 16, color: t.txt, padding: 16 };
  return (
    <Screen
      footer={
        <Button onPress={submit} loading={busy} disabled={!name.trim() || !code.trim()}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>Rejoindre</Text>
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
          style={input}
        />
      </Card>
      {(!preset || !code) && (
      <Card>
        <TextInput
          accessibilityLabel="Code du groupe"
          placeholder="CODE"
          placeholderTextColor={t.mut}
          value={code}
          onChangeText={(v) => setCode(v.toUpperCase())}
          autoCapitalize="characters"
          autoCorrect={false}
          style={[input, { letterSpacing: 6 }]}
        />
      </Card>
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
            style={input}
          />
        </Card>
      )}
      {error && <Text style={{ fontFamily: t.font.semibold, color: t.tag }}>{error}</Text>}
    </Screen>
  );
}
