import { Redirect, router } from 'expo-router';
import { type ComponentRef, useRef, useState } from 'react';
import { TextInput } from 'react-native';

import { AuthScreen, ErrorText, Field, PasswordField, TextLink } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { auth, authError, flow, PASSWORD_MIN, useFeatures } from '@/lib/auth';
import { useSession } from '@/lib/session';
import { useFinishSignIn } from '@/lib/sign-in';

/** Step 2: the password. Signs in when the email has an account, else signs up (first name + password). */
export default function PasswordStep() {
  const { signIn } = useSession();
  const finish = useFinishSignIn();
  const { email: canEmail } = useFeatures();
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const passwordRef = useRef<ComponentRef<typeof TextInput>>(null);
  const existing = flow.account;
  if (!flow.email) return <Redirect href="/join" />;

  const ok = existing ? !!password : !!name.trim() && password.length >= PASSWORD_MIN;
  const submit = async () => {
    if (!ok || busy) return;
    setBusy(true);
    setError(null);
    try {
      if (existing) await finish(await auth.login(flow.email, password));
      else await signIn(await auth.signup(flow.email, password, name.trim()));
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthScreen
      title={existing ? 'Bon retour' : 'Bienvenue'}
      sub={flow.email}
      oneLine
      footer={
        <Button onPress={submit} loading={busy} disabled={!ok}>
          {existing ? 'Se connecter' : 'Créer mon compte'}
        </Button>
      }>
      {!existing && (
        <Field
          placeholder="Prénom"
          value={name}
          onChangeText={setName}
          maxLength={24}
          autoComplete="given-name"
          textContentType="givenName"
          autoFocus
          returnKeyType="next"
          onSubmitEditing={() => passwordRef.current?.focus()}
        />
      )}
      <PasswordField
        ref={passwordRef}
        fresh={!existing}
        placeholder={existing ? 'Mot de passe' : `Mot de passe (${PASSWORD_MIN} min.)`}
        value={password}
        onChangeText={setPassword}
        autoFocus={existing}
        maxLength={128}
        returnKeyType="go"
        onSubmitEditing={submit}
      />
      <ErrorText>{error}</ErrorText>
      {existing && canEmail && <TextLink onPress={() => router.push('/auth/forgot')}>Mot de passe oublié ?</TextLink>}
    </AuthScreen>
  );
}
