import { useState } from 'react';
import { AuthScreen } from '@/components/auth/auth-screen';
import { auth } from '@/lib/api/auth';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { Button } from '@/components/ui/button';
import { CODE_LENGTH, CodeField } from '@/components/ui/field';
import { ErrorText, TextLink } from '@/components/ui/text';
import { useCooldown } from '@/lib/hooks/use-cooldown';
import { flow } from '@/lib/auth-flow';
import { authError } from '@/lib/api/errors';

/** Right after sign-up: the 6-digit code sent to the email. Can wait ("Plus tard", then Profile > Sécurité). */
export default function VerifyStep() {
  const tr = useT();
  const { doneVerifying } = useSession();
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  // The sign-up's e-mail couldn't be sent: said right away (resend below).
  const [error, setError] = useState<string | null>(() => (flow.mailFailed ? tr('errors.email_failed') : null));
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
