import { Redirect } from 'expo-router';
import { type ComponentRef, useRef, useState } from 'react';
import { TextInput } from 'react-native';

import { AuthScreen, CODE_LENGTH, CodeField, ErrorText, PasswordField, TextLink, useCooldown } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { auth, authError, flow, PASSWORD_MIN } from '@/lib/auth';
import { useFinishSignIn } from '@/lib/sign-in';

/** The emailed code and a new password: signed in (or on to 2FA), every other device signed out. */
export default function ResetStep() {
  const finish = useFinishSignIn();
  const [code, setCode] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [wait, startWait] = useCooldown();
  const passwordRef = useRef<ComponentRef<typeof TextInput>>(null);
  if (!flow.email) return <Redirect href="/join" />;

  const ok = code.length === CODE_LENGTH && password.length >= PASSWORD_MIN;
  const submit = async () => {
    if (!ok || busy) return;
    setBusy(true);
    setError(null);
    try {
      await finish(await auth.reset(flow.email, code, password), true);
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(false);
    }
  };
  const resend = async () => {
    setError(null);
    startWait();
    await auth.forgot(flow.email).catch((e) => setError(authError(e)));
  };

  return (
    <AuthScreen
      title="Nouveau mot de passe"
      sub={`Code envoyé à ${flow.email}`}
      footer={
        <Button onPress={submit} loading={busy} disabled={!ok}>
          Valider
        </Button>
      }>
      <CodeField value={code} onChange={setCode} onDone={() => passwordRef.current?.focus()} />
      <PasswordField
        ref={passwordRef}
        fresh
        placeholder={`Nouveau mot de passe (${PASSWORD_MIN} min.)`}
        value={password}
        onChangeText={setPassword}
        maxLength={128}
        returnKeyType="go"
        onSubmitEditing={submit}
      />
      <ErrorText>{error}</ErrorText>
      <TextLink onPress={resend} disabled={wait > 0}>
        {wait > 0 ? `Renvoyer (${wait} s)` : 'Renvoyer le code'}
      </TextLink>
    </AuthScreen>
  );
}
