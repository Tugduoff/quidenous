import { Redirect } from 'expo-router';
import { useState } from 'react';

import { AuthScreen, CODE_LENGTH, CodeField, ErrorText, RecoveryField, TextLink } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { ApiError } from '@/lib/api';
import { auth, authError, flow } from '@/lib/auth';
import { useSession } from '@/lib/session';

/** 2FA sign-in: the code of the authenticator app, or a recovery code. */
export default function TwoFactorStep() {
  const { signIn } = useSession();
  const [recovery, setRecovery] = useState(false);
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!flow.challenge) return <Redirect href="/join" />;

  const ok = recovery ? code.length >= 10 : code.length === CODE_LENGTH;
  const submit = async (value = code) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await signIn(await auth.login2fa(flow.challenge, value));
      flow.challenge = '';
    } catch (e) {
      setError(e instanceof ApiError && e.code === 'expired' ? 'Trop tard, reconnecte-toi' : authError(e));
      setCode('');
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthScreen
      title="Double authentification"
      sub={recovery ? 'Un de tes codes de secours' : "Le code de ton app d'authentification"}
      footer={
        <Button onPress={() => submit()} loading={busy} disabled={!ok}>
          Valider
        </Button>
      }>
      {recovery ? (
        <RecoveryField value={code} onChange={setCode} onDone={() => ok && submit()} />
      ) : (
        <CodeField value={code} onChange={setCode} onDone={submit} />
      )}
      <ErrorText>{error}</ErrorText>
      <TextLink
        onPress={() => {
          setRecovery((r) => !r);
          setCode('');
          setError(null);
        }}>
        {recovery ? "Utiliser l'app d'authentification" : 'Utiliser un code de secours'}
      </TextLink>
    </AuthScreen>
  );
}
