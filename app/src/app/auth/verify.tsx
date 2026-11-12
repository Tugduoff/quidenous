import { useState } from 'react';

import { AuthScreen, CODE_LENGTH, CodeField, ErrorText, TextLink, useCooldown } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { auth, authError, flow } from '@/lib/auth';
import { useSession } from '@/lib/session';

/** Right after sign-up: the 6-digit code sent to the email. Can wait ("Plus tard", then Profile > Sécurité). */
export default function VerifyStep() {
  const { doneVerifying } = useSession();
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [wait, startWait] = useCooldown();

  const submit = async (value = code) => {
    if (busy || value.length !== CODE_LENGTH) return;
    setBusy(true);
    setError(null);
    try {
      await auth.verify(value);
      doneVerifying();
    } catch (e) {
      setError(authError(e));
      setCode('');
    } finally {
      setBusy(false);
    }
  };
  const resend = async () => {
    setError(null);
    startWait();
    await auth.sendVerify().catch((e) => setError(authError(e)));
  };

  return (
    <AuthScreen
      back={false}
      title="Vérifie ton e-mail"
      sub={flow.email ? `Code envoyé à ${flow.email}` : undefined}
      footer={
        <>
          <Button onPress={() => submit()} loading={busy} disabled={code.length !== CODE_LENGTH}>
            Valider
          </Button>
          <TextLink onPress={doneVerifying}>Plus tard</TextLink>
        </>
      }>
      <CodeField value={code} onChange={setCode} onDone={submit} />
      <ErrorText>{error}</ErrorText>
      <TextLink onPress={resend} disabled={wait > 0}>
        {wait > 0 ? `Renvoyer (${wait} s)` : 'Renvoyer'}
      </TextLink>
    </AuthScreen>
  );
}
