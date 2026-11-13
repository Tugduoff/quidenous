import { useState } from 'react';

import { AuthScreen, CODE_LENGTH, CodeField, ErrorText, TextLink, useCooldown } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { auth, authError, flow } from '@/lib/auth';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';

/** Right after sign-up: the 6-digit code sent to the email. Can wait ("Plus tard", then Profile > Sécurité). */
export default function VerifyStep() {
  const tr = useT();
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
      title={tr('auth.verify.title')}
      sub={flow.email ? tr('auth.codeSentTo', { email: flow.email }) : undefined}
      oneLine
      footer={
        <>
          <Button onPress={() => submit()} loading={busy} disabled={code.length !== CODE_LENGTH}>
            {tr('auth.validate')}
          </Button>
          <TextLink onPress={doneVerifying}>{tr('auth.verify.later')}</TextLink>
        </>
      }>
      <CodeField value={code} onChange={setCode} onDone={submit} />
      <ErrorText>{error}</ErrorText>
      <TextLink onPress={resend} disabled={wait > 0}>
        {wait > 0 ? tr('auth.resendIn', { count: wait }) : tr('auth.resend')}
      </TextLink>
    </AuthScreen>
  );
}
