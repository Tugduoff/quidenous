import { router } from 'expo-router';
import { useState } from 'react';

import { AuthScreen, ErrorText, Field } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { auth, authError, flow, looksLikeEmail } from '@/lib/auth';

/** Step 1: the email. The next step signs in or signs up, whichever fits. */
export default function EmailStep() {
  const [email, setEmail] = useState(flow.email);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ok = looksLikeEmail(email);

  const submit = async () => {
    if (!ok || busy) return;
    const value = email.trim().toLowerCase();
    setBusy(true);
    setError(null);
    try {
      const { account } = await auth.email(value);
      flow.email = value;
      flow.account = account;
      router.push('/auth/password');
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthScreen
      title="Ton e-mail"
      footer={
        <Button onPress={submit} loading={busy} disabled={!ok}>
          Continuer
        </Button>
      }>
      <Field
        placeholder="toi@exemple.fr"
        accessibilityLabel="E-mail"
        value={email}
        onChangeText={setEmail}
        keyboardType="email-address"
        inputMode="email"
        autoCapitalize="none"
        autoCorrect={false}
        autoComplete="email"
        textContentType="emailAddress"
        autoFocus
        returnKeyType="next"
        onSubmitEditing={submit}
      />
      <ErrorText>{error}</ErrorText>
    </AuthScreen>
  );
}
