import { Redirect, router } from 'expo-router';
import { type ComponentRef, useRef, useState } from 'react';
import { Text, TextInput } from 'react-native';
import { AuthScreen } from '@/components/auth/auth-screen';
import { TermsCheck } from '@/components/auth/terms';
import { auth, authError, flow, PASSWORD_MIN, useFeatures } from '@/lib/api/auth';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useFinishSignIn } from '@/lib/sign-in';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Field, PasswordField } from '@/components/ui/field';
import { ErrorText, TextLink } from '@/components/ui/text';

/** Step 2: the password. Signs in when the email has an account, else signs up (first name + password). */
export default function PasswordStep() {
  const t = useTheme();
  const tr = useT();
  const { signIn } = useSession();
  const finish = useFinishSignIn();
  const { email: canEmail } = useFeatures();
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [terms, setTerms] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const passwordRef = useRef<ComponentRef<typeof TextInput>>(null);
  const existing = flow.account;
  if (!flow.email) return <Redirect href="/join" />;

  const ok = existing ? !!password : !!name.trim() && password.length >= PASSWORD_MIN && terms;
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
      title={existing ? tr('auth.signIn.welcomeBack') : tr('auth.signIn.welcome')}
      sub={flow.email}
      oneLine
      footer={
        <Button onPress={submit} loading={busy} disabled={!ok}>
          {existing ? tr('auth.signIn.signIn') : tr('auth.signIn.signUp')}
        </Button>
      }>
      {!existing && (
        <Field
          placeholder={tr('auth.name')}
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
        placeholder={existing ? tr('auth.password') : tr('auth.newPassword', { min: PASSWORD_MIN })}
        value={password}
        onChangeText={setPassword}
        autoFocus={existing}
        maxLength={128}
        returnKeyType="go"
        onSubmitEditing={submit}
      />
      {/* Why "Créer mon compte" stays grey: the password is still too short. */}
      {!existing && password.length > 0 && password.length < PASSWORD_MIN && (
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: -4 }}>
          {tr('auth.minLength', { min: PASSWORD_MIN })}
        </Text>
      )}
      {!existing && <TermsCheck value={terms} onChange={setTerms} />}
      <ErrorText>{error}</ErrorText>
      {existing && canEmail && <TextLink onPress={() => router.push('/auth/forgot')}>{tr('auth.signIn.forgot')}</TextLink>}
    </AuthScreen>
  );
}
