import { Redirect, router } from 'expo-router';
import { useState } from 'react';
import { AuthScreen } from '@/components/auth/auth-screen';
import { ApiError } from '@/lib/api/client';
import { auth } from '@/lib/api/auth';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { Button } from '@/components/ui/button';
import { CODE_LENGTH, CodeField, RecoveryField } from '@/components/ui/field';
import { ErrorText, TextLink } from '@/components/ui/text';
import { flow } from '@/lib/auth-flow';
import { authError } from '@/lib/api/errors';
import { useFeatures } from '@/lib/api/features';

export default function TwoFactorStep() {
  const tr = useT();
  const { signIn } = useSession();
  const features = useFeatures();
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
      setError(e instanceof ApiError && e.code === 'expired' ? tr('auth.twoFactor.tooLate') : authError(e));
      setCode('');
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthScreen
      title={tr('auth.twoFactor.title')}
      sub={recovery ? tr('auth.twoFactor.recoverySub') : tr('auth.twoFactor.appSub')}
      footer={
        <Button onPress={() => submit()} loading={busy} disabled={!ok}>
          {tr('auth.validate')}
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
        {recovery ? tr('auth.twoFactor.useApp') : tr('auth.twoFactor.useRecovery')}
      </TextLink>
      {recovery && features.email && features.accountRecovery && (
        <TextLink onPress={() => router.push('/auth/mfa-reset')}>{tr('auth.twoFactor.lost')}</TextLink>
      )}
    </AuthScreen>
  );
}
