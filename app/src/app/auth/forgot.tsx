import { router } from 'expo-router';
import { useState } from 'react';

import { AuthScreen, ErrorText, Field } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { auth, authError, flow, looksLikeEmail } from '@/lib/auth';
import { useT } from '@/lib/i18n';

/** Forgot password: a code by email (the answer never says whether the account exists). */
export default function ForgotStep() {
  const tr = useT();
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
      await auth.forgot(value);
      flow.email = value;
      router.push('/auth/reset');
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthScreen
      title={tr('auth.forgot.title')}
      footer={
        <Button onPress={submit} loading={busy} disabled={!ok}>
          {tr('auth.forgot.submit')}
        </Button>
      }>
      <Field
        placeholder={tr('auth.emailPlaceholder')}
        accessibilityLabel={tr('auth.emailLabel')}
        value={email}
        onChangeText={setEmail}
        keyboardType="email-address"
        inputMode="email"
        autoCapitalize="none"
        autoCorrect={false}
        autoComplete="email"
        textContentType="emailAddress"
        autoFocus={!email}
        returnKeyType="go"
        onSubmitEditing={submit}
      />
      <ErrorText>{error}</ErrorText>
    </AuthScreen>
  );
}
