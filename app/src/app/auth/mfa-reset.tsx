import { Redirect, router } from 'expo-router';
import { useState } from 'react';
import { AuthScreen } from '@/components/auth/auth-screen';
import { ApiError } from '@/lib/api/client';
import { auth } from '@/lib/api/auth';
import { dateFormat, useT } from '@/lib/i18n';
import { Button } from '@/components/ui/button';
import { CODE_LENGTH, CodeField } from '@/components/ui/field';
import { ErrorText, TextLink } from '@/components/ui/text';
import { useCooldown } from '@/lib/hooks/use-cooldown';
import { flow } from '@/lib/auth-flow';
import { authError } from '@/lib/api/errors';

/** 2FA lost with no recovery code (from the 2FA step): a code by e-mail, then 2FA is removed 24 h later. */
export default function MfaResetStep() {
  const tr = useT();
  const [sentTo, setSentTo] = useState<string | null>(null);
  const [code, setCode] = useState('');
  const [resetAt, setResetAt] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [wait, startWait] = useCooldown();
  if (!flow.challenge && resetAt == null) return <Redirect href="/join" />;

  const fail = (e: unknown) => setError(e instanceof ApiError && e.code === 'expired' ? tr('auth.twoFactor.tooLate') : authError(e));
  const send = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      setSentTo((await auth.mfaResetSend(flow.challenge)).email);
      startWait();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };
  const confirm = async (value = code) => {
    if (busy || value.length !== CODE_LENGTH) return;
    setBusy(true);
    setError(null);
    try {
      const r = await auth.mfaResetConfirm(flow.challenge, value);
      flow.challenge = '';
      setResetAt(r.reset_at);
    } catch (e) {
      fail(e);
      setCode('');
    } finally {
      setBusy(false);
    }
  };

  if (resetAt != null) {
    const when = dateFormat({ weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' }).format(new Date(resetAt * 1000));
    return (
      <AuthScreen
        back={false}
        title={tr('auth.mfaReset.done')}
        sub={tr('auth.mfaReset.doneSub', { when })}
        footer={<Button onPress={() => router.replace('/join')}>{tr('auth.mfaReset.ok')}</Button>}
      />
    );
  }

  return (
    <AuthScreen
      title={tr('auth.mfaReset.title')}
      sub={sentTo ? tr('auth.codeSentTo', { email: sentTo }) : tr('auth.mfaReset.sub')}
      footer={
        sentTo ? (
          <Button onPress={() => confirm()} loading={busy} disabled={code.length !== CODE_LENGTH}>
            {tr('auth.validate')}
          </Button>
        ) : (
          <Button onPress={send} loading={busy}>
            {tr('auth.mfaReset.send')}
          </Button>
        )
      }>
      {sentTo && <CodeField value={code} onChange={setCode} onDone={confirm} />}
      <ErrorText>{error}</ErrorText>
      {sentTo && (
        <TextLink onPress={send} disabled={wait > 0 || busy}>
          {wait > 0 ? tr('auth.resendIn', { count: wait }) : tr('auth.resend')}
        </TextLink>
      )}
    </AuthScreen>
  );
}
